#pragma once

#include <napi/env.h>

#include <functional>
#include <mutex>

namespace Babylon
{
    class JsRuntime
    {
    public:
        static constexpr auto JS_NATIVE_NAME = "_native";
        using DispatchFunctionT = std::function<void(std::function<void(Napi::Env)>)>;

        static void Initialize(Napi::Env, DispatchFunctionT);
        static JsRuntime& GetFromJavaScript(Napi::Env);
        void Dispatch(std::function<void(Napi::Env)>);

    protected:
        // Note: It is the contract of JsRuntime that its dispatch function must be usable
        // at the moment of construction. JsRuntime cannot be built with dispatch function
        // that captures a refence to a not-yet-completed object that will be completed
        // later -- an instance of an inheriting type, for example. The dispatch function
        // must be safely callable as soon as it is passed to the JsRuntime constructor.
        JsRuntime(DispatchFunctionT);
        JsRuntime(const JsRuntime&) = delete;
        JsRuntime(JsRuntime&&) = delete;

    private:
        using DeleterT = std::function<void(Napi::Env, JsRuntime*)>;
        JsRuntime(DispatchFunctionT, DeleterT);

        DispatchFunctionT m_dispatchFunction{};
        std::mutex m_mutex{};
    };
}
