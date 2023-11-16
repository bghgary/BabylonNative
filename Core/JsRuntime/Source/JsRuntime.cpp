#include <Babylon/JsRuntime.h>
#include <arcana/containers/weak_table.h>

// JsRuntime::NativeObject
namespace Babylon
{
    namespace
    {
        static constexpr auto JS_NATIVE_NAME = "_native";
    }

    Napi::Object JsRuntime::NativeObject::GetFromJavaScript(Napi::Env env)
    {
        return env.Global().Get(JS_NATIVE_NAME).As<Napi::Object>();
    }
}

// JsRuntime::AsyncToken
namespace Babylon
{
    JsRuntime::AsyncToken::AsyncToken(JsRuntime& runtime)
        : m_runtime{&runtime}
    {
        ++m_runtime->m_numAsyncOperations;
    }

    JsRuntime::AsyncToken::~AsyncToken()
    {
        if (m_runtime)
        {
            --m_runtime->m_numAsyncOperations;
        }
    }

    JsRuntime::AsyncToken::AsyncToken(AsyncToken&& other) noexcept
        : m_runtime{other.m_runtime}
    {
        other.m_runtime = nullptr;
    }

    JsRuntime::AsyncToken& JsRuntime::AsyncToken::operator=(AsyncToken&& other) noexcept
    {
        AsyncToken::~AsyncToken();
        std::swap(m_runtime, other.m_runtime);
        return *this;
    }
}

// JsRuntime
namespace Babylon
{
    namespace
    {
        static constexpr auto JS_RUNTIME_NAME = "runtime";
        static constexpr auto JS_WINDOW_NAME = "window";
    }

    JsRuntime::JsRuntime(Napi::Env env, DispatchFunctionT dispatchFunction)
        : m_dispatchFunction{std::move(dispatchFunction)}
    {
        auto global = env.Global();

        if (global.Get(JS_WINDOW_NAME).IsUndefined())
        {
            global.Set(JS_WINDOW_NAME, global);
        }
    }

    JsRuntime& JsRuntime::CreateForJavaScript(Napi::Env env, DispatchFunctionT dispatchFunction)
    {
        auto jsNative = Napi::Object::New(env);
        env.Global().Set(JS_NATIVE_NAME, jsNative);

        auto* runtime = new JsRuntime(env, std::move(dispatchFunction));
        Napi::Value jsRuntime = Napi::External<JsRuntime>::New(env, runtime, [](Napi::Env, JsRuntime* runtime) {
            delete runtime;
        });
        jsNative.Set(JS_RUNTIME_NAME, jsRuntime);
        return *runtime;
    }

    JsRuntime& JsRuntime::GetFromJavaScript(Napi::Env env)
    {
        return *NativeObject::GetFromJavaScript(env)
                    .As<Napi::Object>()
                    .Get(JS_RUNTIME_NAME)
                    .As<Napi::External<JsRuntime>>()
                    .Data();
    }

    JsRuntime::AsyncToken JsRuntime::GetAsyncToken(Napi::Env env)
    {
        return {JsRuntime::GetFromJavaScript(env)};
    }

    void JsRuntime::Dispatch(std::function<void(Napi::Env)> function)
    {
        m_dispatchFunction([function = std::move(function)](Napi::Env env) {
            function(env);

            // The environment will be in a pending exceptional state if
            // Napi::Error::ThrowAsJavaScriptException is invoked within the
            // previous function. Throw and clear the pending exception here to
            // bubble up the exception to the the dispatcher.
            if (env.IsExceptionPending())
            {
                throw env.GetAndClearPendingException();
            }
        });
    }

    void JsRuntime::Shutdown()
    {
        while (m_numAsyncOperations > 0)
        {
            using namespace std::chrono_literals;
            std::this_thread::sleep_for(100ms);
        }
    }
}
