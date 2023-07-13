#include "AppRuntimeImpl.h"

#include <Windows.h>

#include <exception>
#include <sstream>

namespace Babylon
{
    void AppRuntimeImpl::RunPlatformTier()
    {
        RunEnvironmentTier();
    }

    void AppRuntimeImpl::DefaultUnhandledExceptionHandler(const std::exception& error)
    {
        std::stringstream ss{};
        ss << "Uncaught Error: " << error.what() << std::endl;
        OutputDebugStringA(ss.str().data());
    }

    void AppRuntimeImpl::Execute(Dispatchable<void()> callback)
    {
        callback();
    }
}
