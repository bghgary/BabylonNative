#pragma once

#include <Babylon/JsRuntimeScheduler.h>

#include <napi/napi.h>
#include <UrlLib/UrlLib.h>

#include <unordered_map>

namespace Babylon::Polyfills::Internal
{
    class Require final : public Napi::ObjectWrap<Require>
    {
    public:
        static void Initialize(Napi::Env env, std::vector<std::string> rootUrls);

        explicit Require(const Napi::CallbackInfo& info);

    private:
    };
}
