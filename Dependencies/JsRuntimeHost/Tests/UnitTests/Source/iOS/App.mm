#include "../App.h"
#include "Babylon/DebugTrace.h"
#include "ConsoleProbe.h"
#import <Foundation/Foundation.h>

int main()
{
    Babylon::DebugTrace::EnableDebugTrace(true);
    Babylon::DebugTrace::SetTraceOutput([](const char* trace) { NSLog(@"%s", trace); });

    if (!RedirectToReadOnlyForControl("IOS_CONSOLE_PROBE_BAD_STDOUT", STDOUT_FILENO))
    {
        return 1;
    }

    testing::InitGoogleTest();
    auto* probe = new ConsoleProbe{};
    testing::UnitTest::GetInstance()->listeners().Append(probe);
    probe->Record(ProbeStage::BeforeTests);

    int exitCode = RunTests();
    probe->Record(ProbeStage::AfterTests);
    if (!RedirectToReadOnlyForControl("IOS_CONSOLE_PROBE_BAD_STDERR", STDERR_FILENO))
    {
        exitCode = 1;
    }

    // CI will pick up the exit code from stderr when running in the iOS Simulator.
    std::cerr << exitCode << std::endl;
    std::cerr.flush();
    probe->Record(ProbeStage::AfterMarker);
    probe->Report(exitCode);

    return exitCode;
}
