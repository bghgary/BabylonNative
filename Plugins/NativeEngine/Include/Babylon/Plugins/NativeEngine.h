#pragma once

#include <napi/env.h>

namespace Babylon::Plugins::NativeEngine
{
    void Initialize(Napi::Env env);

    struct NativeTexture
    {
        // Must be called from the JS thread.
        static Napi::Value Wrap(Napi::Env env, uint16_t textureHandle, uint32_t width, uint32_t height);
    };
}
