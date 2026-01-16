#pragma once

#include <Babylon/AppRuntime.h>
#include <Babylon/Graphics/Device.h>
#include <Babylon/Polyfills/Canvas.h>
#include <Babylon/Plugins/NativeInput.h>

#include <optional>
#include <vector>
#include <string>

class Context
{
public:
    Context(Babylon::Graphics::WindowT window, size_t width, size_t height, const std::vector<std::string>& scripts);
    ~Context();

    // Copy semantics
    Context(const Context&) = delete;
    Context& operator=(const Context&) noexcept = delete;

    // Move semantics
    Context(Context&&) = delete;
    Context& operator=(Context&&) noexcept = delete;

    Babylon::Graphics::Device& Device() { return *m_device; }
    Babylon::Graphics::DeviceUpdate& DeviceUpdate() { return *m_deviceUpdate; }
    Babylon::AppRuntime& Runtime() { return *m_runtime; }
    Babylon::Polyfills::Canvas& Canvas() { return *m_canvas; }
    Babylon::Plugins::NativeInput& Input() { return *m_input; }

private:
    std::optional<Babylon::Graphics::Device> m_device;
    std::optional<Babylon::Graphics::DeviceUpdate> m_deviceUpdate;
    std::optional<Babylon::AppRuntime> m_runtime;
    std::optional<Babylon::Polyfills::Canvas> m_canvas{};
    Babylon::Plugins::NativeInput* m_input{};
};
