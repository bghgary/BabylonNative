#include "Context.h"

#include <Babylon/AppRuntime.h>
#include <Babylon/DebugTrace.h>
#include <Babylon/Graphics/Device.h>
#include <Babylon/ScriptLoader.h>
#include <Babylon/ShaderCache.h>

#include <Babylon/Plugins/NativeCamera.h>
#include <Babylon/Plugins/NativeCapture.h>
#include <Babylon/Plugins/NativeEncoding.h>
#include <Babylon/Plugins/NativeEngine.h>
#include <Babylon/Plugins/NativeInput.h>
#include <Babylon/Plugins/NativeOptimizations.h>
#include <Babylon/Plugins/TestUtils.h>

#include <Babylon/Polyfills/Blob.h>
#include <Babylon/Polyfills/Canvas.h>
#include <Babylon/Polyfills/Console.h>
#include <Babylon/Polyfills/Window.h>
#include <Babylon/Polyfills/XMLHttpRequest.h>

#include <iostream>
#include <sstream>

namespace
{
    const char* GetLogLevelString(Babylon::Polyfills::Console::LogLevel logLevel)
    {
        switch (logLevel)
        {
            case Babylon::Polyfills::Console::LogLevel::Log:
                return "Log";
            case Babylon::Polyfills::Console::LogLevel::Warn:
                return "Warn";
            case Babylon::Polyfills::Console::LogLevel::Error:
                return "Error";
            default:
                return "";
        }
    }
}

Context::Context(Babylon::Graphics::WindowT window, size_t width, size_t height, const std::vector<std::string>& scripts)
{
    Babylon::DebugTrace::EnableDebugTrace(true);
    Babylon::DebugTrace::SetTraceOutput([](const char* trace) {
        std::ostringstream ss{};
        ss << "[Uncaught Error] " << trace << std::endl;

        //OutputDebugStringA(ss.str().data());
        std::cout << ss.str();
        std::cout.flush();
    });

    Babylon::Graphics::Configuration graphicsConfig{};
    graphicsConfig.Window = window;
    graphicsConfig.Width = width;
    graphicsConfig.Height = height;
    graphicsConfig.MSAASamples = 4;

    m_device.emplace(graphicsConfig);
    m_deviceUpdate.emplace(m_device->GetUpdate("update"));

    Babylon::ShaderCache::Enabled(true);

    m_device->StartRenderingCurrentFrame();
    m_deviceUpdate->Start();

    Babylon::AppRuntime::Options options{};

    options.EnableDebugger = true;

    options.UnhandledExceptionHandler = [](const Napi::Error& error) {
        std::ostringstream ss{};
        ss << "[Uncaught Error] " << Napi::GetErrorString(error) << std::endl;

        //OutputDebugStringA(ss.str().data());
        std::cerr << ss.str();
        std::cerr.flush();

        std::quick_exit(1);
    };

    m_runtime.emplace(options);

    m_runtime->Dispatch([this, window](Napi::Env env) {
        m_device->AddToJavaScript(env);

        Babylon::Polyfills::Blob::Initialize(env);

        Babylon::Polyfills::Console::Initialize(env, [](const char* message, Babylon::Polyfills::Console::LogLevel logLevel) {
            std::ostringstream ss{};
            ss << "[" << GetLogLevelString(logLevel) << "] " << message << std::endl;

            //OutputDebugStringA(ss.str().data());
            std::cout << ss.str();
            std::cout.flush();
        });

        Babylon::Polyfills::Window::Initialize(env);

        Babylon::Polyfills::XMLHttpRequest::Initialize(env);

        m_canvas.emplace(Babylon::Polyfills::Canvas::Initialize(env));

        Babylon::Plugins::NativeEncoding::Initialize(env);

        Babylon::Plugins::NativeEngine::Initialize(env);

        Babylon::Plugins::NativeOptimizations::Initialize(env);

        Babylon::Plugins::NativeCapture::Initialize(env);

        Babylon::Plugins::NativeCamera::Initialize(env);

        m_input = &Babylon::Plugins::NativeInput::CreateForJavaScript(env);

        Babylon::Plugins::TestUtils::Initialize(env, window);
    });

    Babylon::ScriptLoader loader{*m_runtime};
    loader.LoadScript("app:///Scripts/ammo.js");
    // Commenting out recast.js for now because v8jsi is incompatible with asm.js.
    // loader.LoadScript("app:///Scripts/recast.js");
    loader.LoadScript("app:///Scripts/babylon.max.js");
    loader.LoadScript("app:///Scripts/babylonjs.loaders.js");
    loader.LoadScript("app:///Scripts/babylonjs.materials.js");
    loader.LoadScript("app:///Scripts/babylon.gui.js");
    loader.LoadScript("app:///Scripts/meshwriter.min.js");
    loader.LoadScript("app:///Scripts/babylonjs.serializers.js");

    for (const auto& script : scripts)
    {
        loader.LoadScript(script);
    }
}

Context::~Context()
{
    if (m_device)
    {
        m_deviceUpdate->Finish();
        m_device->FinishRenderingCurrentFrame();
    }

    m_canvas.reset();
    m_input = {};
    m_runtime.reset();
    m_deviceUpdate.reset();
    m_device.reset();
}
