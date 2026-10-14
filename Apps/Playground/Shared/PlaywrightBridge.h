#pragma once

#include <Windows.h>

#include <cstdint>
#include <mutex>
#include <optional>
#include <queue>
#include <string>
#include <string_view>
#include <thread>

namespace Playground
{
    class PlaywrightBridge
    {
    public:
        ~PlaywrightBridge();

        enum class Method
        {
            Evaluate,
            Resize,
            Screenshot,
            Close,
        };

        struct Command
        {
            uint64_t Id{};
            Method CommandMethod{};
            std::string Payload;
            uint32_t Width{};
            uint32_t Height{};
        };

        static constexpr UINT CommandMessage{WM_APP + 1};

        void Start(HWND window);
        std::optional<Command> TakeCommand();

        void SendReady();
        void SendConsole(std::string_view level, std::string_view message);
        void SendResult(uint64_t id, std::string_view payload);
        void SendBinaryResult(uint64_t id, const uint8_t* data, size_t size);
        void SendError(uint64_t id, std::string_view message);

    private:
        void ReadLoop(HWND window);
        void SendLine(std::string line);

        std::mutex m_commandMutex;
        std::queue<Command> m_commands;
        std::mutex m_outputMutex;
        std::thread m_readerThread;
    };
}
