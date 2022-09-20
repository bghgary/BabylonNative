#include <napi/env.h>

namespace Napi
{
    Value Eval(Env env, const char* source, const char* sourceUrl)
    {
        napi_value result;
        if (napi_run_script(env, String::New(env, source), sourceUrl, &result) != napi_ok)
        {
            throw env.GetAndClearPendingException();
        }
        return{ env, result };
    }
}
