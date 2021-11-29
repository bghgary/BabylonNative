#include <Babylon/Plugins/NativeEngine.h>
#include "NativeDataStream.h"
#include "NativeEngine.h"

namespace Babylon::Plugins::NativeEngine
{
    void Initialize(Napi::Env env)
    {
        Babylon::NativeDataStream::Initialize(env);
        Babylon::NativeEngine::Initialize(env);
    }

    Napi::Value NativeTexture::Wrap(Napi::Env env, uint16_t textureHandle, uint32_t width, uint32_t height)
    {
        return Babylon::NativeEngine::WrapNativeTexture(env, textureHandle, width, height);
    }
}
