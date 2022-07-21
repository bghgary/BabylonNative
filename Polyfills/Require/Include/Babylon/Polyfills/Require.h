#pragma once

#include <napi/env.h>

namespace Babylon::Polyfills::Require
{
    void Initialize(Napi::Env env, std::string searchPath);
}
