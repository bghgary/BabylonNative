#include "PlaywrightBridge.h"

#include <algorithm>
#include <array>
#include <charconv>
#include <cstdio>
#include <iostream>
#include <vector>

namespace
{
    constexpr std::string_view ProtocolPrefix{"PW\t"};
    constexpr char Base64Alphabet[]{"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/"};

    std::string Base64Encode(const uint8_t* data, size_t size)
    {
        std::string result;
        result.reserve(((size + 2) / 3) * 4);

        for (size_t index = 0; index < size; index += 3)
        {
            const uint32_t first = data[index];
            const uint32_t second = index + 1 < size ? data[index + 1] : 0;
            const uint32_t third = index + 2 < size ? data[index + 2] : 0;
            const uint32_t value = (first << 16) | (second << 8) | third;

            result.push_back(Base64Alphabet[(value >> 18) & 0x3f]);
            result.push_back(Base64Alphabet[(value >> 12) & 0x3f]);
            result.push_back(index + 1 < size ? Base64Alphabet[(value >> 6) & 0x3f] : '=');
            result.push_back(index + 2 < size ? Base64Alphabet[value & 0x3f] : '=');
        }

        return result;
    }

    std::optional<std::vector<uint8_t>> Base64Decode(std::string_view value)
    {
        if (value.size() % 4 != 0)
        {
            return std::nullopt;
        }

        std::array<int8_t, 256> lookup{};
        lookup.fill(-1);
        for (int8_t index = 0; index < 64; ++index)
        {
            lookup[static_cast<uint8_t>(Base64Alphabet[index])] = index;
        }

        std::vector<uint8_t> result;
        result.reserve((value.size() / 4) * 3);
        for (size_t index = 0; index < value.size(); index += 4)
        {
            uint32_t accumulator{};
            int padding{};
            for (size_t offset = 0; offset < 4; ++offset)
            {
                const auto character = static_cast<uint8_t>(value[index + offset]);
                if (character == '=')
                {
                    ++padding;
                    accumulator <<= 6;
                }
                else if (lookup[character] >= 0 && padding == 0)
                {
                    accumulator = (accumulator << 6) | static_cast<uint8_t>(lookup[character]);
                }
                else
                {
                    return std::nullopt;
                }
            }

            result.push_back(static_cast<uint8_t>((accumulator >> 16) & 0xff));
            if (padding < 2)
            {
                result.push_back(static_cast<uint8_t>((accumulator >> 8) & 0xff));
            }
            if (padding < 1)
            {
                result.push_back(static_cast<uint8_t>(accumulator & 0xff));
            }
        }

        return result;
    }

    std::vector<std::string_view> Split(std::string_view line)
    {
        std::vector<std::string_view> fields;
        size_t start{};
        while (start <= line.size())
        {
            const auto end = line.find('\t', start);
            fields.emplace_back(line.substr(start, end == std::string_view::npos ? end : end - start));
            if (end == std::string_view::npos)
            {
                break;
            }
            start = end + 1;
        }
        return fields;
    }

    template<typename T>
    bool ParseInteger(std::string_view value, T& result)
    {
        const auto parsed = std::from_chars(value.data(), value.data() + value.size(), result);
        return parsed.ec == std::errc{} && parsed.ptr == value.data() + value.size();
    }
}

namespace Playground
{
    PlaywrightBridge::~PlaywrightBridge()
    {
        if (m_readerThread.joinable())
        {
            CancelSynchronousIo(m_readerThread.native_handle());
            m_readerThread.join();
        }
    }

    void PlaywrightBridge::Start(HWND window)
    {
        if (m_readerThread.joinable())
        {
            return;
        }
        m_readerThread = std::thread{[this, window] {
            ReadLoop(window);
        }};
    }

    std::optional<PlaywrightBridge::Command> PlaywrightBridge::TakeCommand()
    {
        std::scoped_lock lock{m_commandMutex};
        if (m_commands.empty())
        {
            return std::nullopt;
        }

        auto command = std::move(m_commands.front());
        m_commands.pop();
        return command;
    }

    void PlaywrightBridge::SendReady()
    {
        SendLine("PW\t0\tREADY");
    }

    void PlaywrightBridge::SendConsole(std::string_view level, std::string_view message)
    {
        const auto encoded = Base64Encode(reinterpret_cast<const uint8_t*>(message.data()), message.size());
        SendLine("PW\t0\tCONSOLE\t" + std::string{level} + "\t" + encoded);
    }

    void PlaywrightBridge::SendResult(uint64_t id, std::string_view payload)
    {
        const auto encoded = Base64Encode(reinterpret_cast<const uint8_t*>(payload.data()), payload.size());
        SendLine("PW\t" + std::to_string(id) + "\tOK\t" + encoded);
    }

    void PlaywrightBridge::SendBinaryResult(uint64_t id, const uint8_t* data, size_t size)
    {
        SendLine("PW\t" + std::to_string(id) + "\tOK\t" + Base64Encode(data, size));
    }

    void PlaywrightBridge::SendError(uint64_t id, std::string_view message)
    {
        const auto encoded = Base64Encode(reinterpret_cast<const uint8_t*>(message.data()), message.size());
        SendLine("PW\t" + std::to_string(id) + "\tERROR\t" + encoded);
    }

    void PlaywrightBridge::ReadLoop(HWND window)
    {
        std::string line;
        while (std::getline(std::cin, line))
        {
            if (!line.starts_with(ProtocolPrefix))
            {
                continue;
            }

            const auto fields = Split(line);
            uint64_t id{};
            if (fields.size() < 3 || !ParseInteger(fields[1], id))
            {
                continue;
            }

            Command command{};
            command.Id = id;
            if (fields[2] == "EVALUATE" && fields.size() == 4)
            {
                const auto decoded = Base64Decode(fields[3]);
                if (!decoded)
                {
                    SendError(id, "Invalid base64 evaluate payload");
                    continue;
                }
                command.CommandMethod = Method::Evaluate;
                command.Payload.assign(decoded->begin(), decoded->end());
            }
            else if (fields[2] == "RESIZE" && fields.size() == 5 &&
                     ParseInteger(fields[3], command.Width) && ParseInteger(fields[4], command.Height))
            {
                command.CommandMethod = Method::Resize;
            }
            else if (fields[2] == "SCREENSHOT" && fields.size() == 3)
            {
                command.CommandMethod = Method::Screenshot;
            }
            else if (fields[2] == "CLOSE" && fields.size() == 3)
            {
                command.CommandMethod = Method::Close;
            }
            else
            {
                SendError(id, "Unknown or malformed command");
                continue;
            }

            {
                std::scoped_lock lock{m_commandMutex};
                m_commands.emplace(std::move(command));
            }
            PostMessage(window, CommandMessage, 0, 0);
        }

        PostMessage(window, WM_CLOSE, 0, 0);
    }

    void PlaywrightBridge::SendLine(std::string line)
    {
        std::scoped_lock lock{m_outputMutex};
        line.push_back('\n');
        std::fwrite(line.data(), 1, line.size(), stdout);
        std::fflush(stdout);
    }
}
