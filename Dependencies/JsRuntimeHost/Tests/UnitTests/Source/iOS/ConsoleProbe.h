#pragma once

#include <gtest/gtest.h>
#include <array>
#include <cerrno>
#include <cstddef>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <iostream>

#include <fcntl.h>
#include <os/log.h>
#include <unistd.h>

namespace
{
    enum class ProbeStage
    {
        BeforeTests,
        AfterJavaScript,
        BeforeLogger,
        AfterLogger,
        AfterTests,
        AfterMarker,
        Count,
    };

    struct StreamSnapshot
    {
        bool recorded{};
        int testResult{-1};
        int coutState{};
        int cerrState{};
        int stdoutError{};
        int stderrError{};
        int stdoutFlags{-1};
        int stdoutFlagsError{};
        int stderrFlags{-1};
        int stderrFlagsError{};
    };

    StreamSnapshot Snapshot(int testResult = -1)
    {
        const int savedErrno = errno;
        StreamSnapshot result{};
        result.recorded = true;
        result.testResult = testResult;
        result.coutState = static_cast<int>(std::cout.rdstate());
        result.cerrState = static_cast<int>(std::cerr.rdstate());
        result.stdoutError = std::ferror(stdout);
        result.stderrError = std::ferror(stderr);
        result.stdoutFlags = ::fcntl(STDOUT_FILENO, F_GETFL);
        result.stdoutFlagsError = result.stdoutFlags == -1 ? errno : 0;
        result.stderrFlags = ::fcntl(STDERR_FILENO, F_GETFL);
        result.stderrFlagsError = result.stderrFlags == -1 ? errno : 0;
        errno = savedErrno;
        return result;
    }

    bool IsTest(const testing::TestInfo& info, const char* suite, const char* name)
    {
        return std::strcmp(info.test_suite_name(), suite) == 0 &&
            std::strcmp(info.name(), name) == 0;
    }

    class ConsoleProbe final : public testing::EmptyTestEventListener
    {
    public:
        void Record(ProbeStage stage, int testResult = -1)
        {
            m_samples[static_cast<std::size_t>(stage)] = Snapshot(testResult);
        }

        void OnTestStart(const testing::TestInfo& info) override
        {
            if (IsTest(info, "StandardStreamLogger", "Lifecycle"))
            {
                Record(ProbeStage::BeforeLogger);
            }
        }

        void OnTestEnd(const testing::TestInfo& info) override
        {
            const int result = info.result()->Skipped() ? 2 : (info.result()->Failed() ? 1 : 0);
            if (IsTest(info, "JavaScript", "All"))
            {
                Record(ProbeStage::AfterJavaScript, result);
            }
            else if (IsTest(info, "StandardStreamLogger", "Lifecycle"))
            {
                Record(ProbeStage::AfterLogger, result);
            }
        }

        void Report(int exitCode) const
        {
            constexpr std::array<const char*, static_cast<std::size_t>(ProbeStage::Count)> labels{
                "before", "javascript", "logger_before", "logger_after", "tests", "marker"};
            std::array<char, 2048> message{};
            int written = std::snprintf(message.data(), message.size(), "pid=%ld exit=%d",
                static_cast<long>(::getpid()), exitCode);
            if (written < 0 || static_cast<std::size_t>(written) >= message.size())
            {
                os_log_error(OS_LOG_DEFAULT, "IOS_CONSOLE_PROBE_V1 report formatting failed");
                return;
            }

            std::size_t offset = static_cast<std::size_t>(written);
            for (std::size_t i = 0; i < m_samples.size(); ++i)
            {
                const auto& sample = m_samples[i];
                written = std::snprintf(message.data() + offset, message.size() - offset,
                    " %s=%d,%d,%d,%d,%d,%d,%d,%d,%d,%d",
                    labels[i], sample.recorded ? 1 : 0, sample.testResult,
                    sample.coutState, sample.cerrState, sample.stdoutError, sample.stderrError,
                    sample.stdoutFlags, sample.stdoutFlagsError, sample.stderrFlags, sample.stderrFlagsError);
                if (written < 0 || static_cast<std::size_t>(written) >= message.size() - offset)
                {
                    os_log_error(OS_LOG_DEFAULT, "IOS_CONSOLE_PROBE_V1 report formatting failed");
                    return;
                }
                offset += static_cast<std::size_t>(written);
            }

            os_log_with_type(OS_LOG_DEFAULT, OS_LOG_TYPE_DEFAULT,
                "IOS_CONSOLE_PROBE_V1 %{public}s", message.data());
        }

    private:
        std::array<StreamSnapshot, static_cast<std::size_t>(ProbeStage::Count)> m_samples{};
    };

    bool RedirectToReadOnlyForControl(const char* variable, int target)
    {
        if (std::getenv(variable) == nullptr)
        {
            return true;
        }

        const int source = ::open("/dev/null", O_RDONLY);
        if (source < 0)
        {
            os_log_error(OS_LOG_DEFAULT, "IOS_CONSOLE_PROBE_V1 control open failed: %d", errno);
            return false;
        }
        if (source != target)
        {
            const int result = ::dup2(source, target);
            const int savedErrno = errno;
            ::close(source);
            if (result < 0)
            {
                os_log_error(OS_LOG_DEFAULT, "IOS_CONSOLE_PROBE_V1 control dup2 failed: %d", savedErrno);
                return false;
            }
        }
        return true;
    }
}
