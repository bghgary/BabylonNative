#pragma once

#include <napi/env.h>

#include <functional>
#include <mutex>

namespace Babylon
{
    class JsRuntime final
    {
    public:
        // Copy semantics
        JsRuntime(const JsRuntime&) = delete;
        JsRuntime& operator=(const JsRuntime&) = delete;

        // Move semantics
        JsRuntime(JsRuntime&&) noexcept = delete;
        JsRuntime& operator=(JsRuntime&&) noexcept = delete;

        class NativeObject final
        {
        public:
            static Napi::Object GetFromJavaScript(Napi::Env);
        };

        class Token final
        {
        public:
            ~Token();

            // Move semantics
            Token(Token&&) noexcept;
            Token& operator=(Token&&) noexcept;

            // Copy semantics
            Token(const Token&) = delete;
            const Token& operator=(const Token&) = delete;

        private:
            friend class JsRuntime;

            Token(JsRuntime& runtime);

            JsRuntime* m_runtime;
        };

        class AsyncToken final
        {
        public:
            ~AsyncToken();

            // Move semantics
            AsyncToken(AsyncToken&&) noexcept;
            AsyncToken& operator=(AsyncToken&&) noexcept;

            // Copy semantics
            AsyncToken(const AsyncToken&) = delete;
            const AsyncToken& operator=(const AsyncToken&) = delete;

        private:
            friend class JsRuntime;

            AsyncToken(JsRuntime& runtime);

            JsRuntime* m_runtime;
        };

        //class EventToken final
        //{
        //public:
        //    ~EventToken();

        //    // Move semantics
        //    EventToken(EventToken&&) noexcept;
        //    EventToken& operator=(EventToken&&) noexcept;

        //    // Copy semantics
        //    EventToken(const EventToken&) = delete;
        //    const EventToken& operator=(const EventToken&) = delete;

        //private:

        //};

        // Any JavaScript errors that occur will bubble up as a Napi::Error C++ exception.
        // JsRuntime expects the provided dispatch function to handle this exception,
        // such as with a try/catch and logging the exception message.
        using DispatchFunctionT = std::function<void(std::function<void(Napi::Env)>)>;

        // Creates the JsRuntime object owned by the JavaScript environment.
        // Note: It is the contract of JsRuntime that its dispatch function must be usable
        // at the moment of construction. JsRuntime cannot be built with dispatch function
        // that captures a reference to a not-yet-completed object that will be completed
        // later -- an instance of an inheriting type, for example. The dispatch function
        // must be safely callable as soon as it is passed to the JsRuntime constructor.
        static JsRuntime& CreateForJavaScript(Napi::Env, DispatchFunctionT);

        // Gets the JsRuntime from the given N-API environment.
        static JsRuntime& GetFromJavaScript(Napi::Env);

        static AsyncToken GetAsyncToken(Napi::Env);

        // Dispatches work onto the JavaScript thread and provides access to the N-API
        // environment.
        void Dispatch(std::function<void(Napi::Env)>);

        //void Dispose();

        //EventToken Disposing(std::function<void()>);

    private:
        JsRuntime(Napi::Env, DispatchFunctionT);

        DispatchFunctionT m_dispatchFunction{};

        std::atomic<uint64_t> m_numAsyncOperations{};
    };
}
