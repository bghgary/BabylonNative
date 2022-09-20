#include <Babylon/Polyfills/Require.h>
#include <Babylon/JsRuntime.h>
#include <filesystem>
#include <fstream>
#include <gsl/gsl>

namespace
{
    std::string readFile(std::filesystem::path filePath)
    {
        const auto size = std::filesystem::file_size(filePath);
        std::string result(size, '\0');

        std::ifstream file{filePath, std::ios::in};
        file.read(result.data(), size);

        return result;
    }

    class RequireHelper
    {
    public:
        RequireHelper(Napi::Env env)
            : m_jsonParse{Napi::Persistent(env.Global().Get("JSON").As<Napi::Object>().Get("parse").As<Napi::Function>())}
            , m_evalHelper{Napi::Persistent(Napi::Eval(env, "(exports, contents) => eval(contents)", "").As<Napi::Function>())}
        {
        }

        Napi::Value Execute(Napi::Env env, std::filesystem::path searchPath, std::filesystem::path path)
        {
            std::filesystem::path resolvedPath;

            if (path.native()[0] == L'.') {
                if (m_currentScriptPath.empty()) {
                    throw Napi::Error::New(env, "Cannot load from a relative path from the root");
                }

                resolvedPath = (m_currentScriptPath.parent_path() / path).lexically_normal();
            }
            else if (path.native()[0] == L'/') {
                resolvedPath = (searchPath / path.native().substr(1)).lexically_normal();
            }
            else
            {
                resolvedPath = (searchPath / path).lexically_normal();
            }

            if (std::filesystem::is_directory(resolvedPath))
            {
                bool foundMain = false;
                std::filesystem::path packageJsonPath{resolvedPath / "package.json"};
                if (std::filesystem::exists(packageJsonPath))
                {
                    Napi::Value main = m_jsonParse.Call({Napi::String::From(env, readFile(packageJsonPath))}).As<Napi::Object>().Get("main");
                    if (!main.IsUndefined())
                    {
                        resolvedPath = (resolvedPath / main.As<Napi::String>().Utf8Value()).lexically_normal();
                        foundMain = true;
                    }
                }

                if (!foundMain)
                {
                    resolvedPath /= "index.js";
                }
            }
            else if (resolvedPath.extension() != ".js" && !std::filesystem::exists(resolvedPath))
            {
                resolvedPath += ".js";
            }

            if (!std::filesystem::exists(resolvedPath))
            {
                return env.Null();
            }

            if (resolvedPath.extension() == ".json")
            {
                return m_jsonParse.Call({Napi::String::From(env, readFile(resolvedPath))});
            }

            std::filesystem::path previousScriptPath = std::move(m_currentScriptPath);
            m_currentScriptPath = resolvedPath;
            auto scope = gsl::finally([this, oldScriptPath = std::move(previousScriptPath)]() mutable {
                m_currentScriptPath = std::move(oldScriptPath);
            });

            Napi::Object exports = Napi::Object::New(env);
            m_evalHelper.Call({exports, Napi::String::New(env, readFile(resolvedPath).data())});
            return exports;
        }

    private:
        const Napi::FunctionReference m_jsonParse;
        const Napi::FunctionReference m_evalHelper;
        
        std::filesystem::path m_currentScriptPath;
    };
}

namespace Babylon::Polyfills::Require
{
    void Initialize(Napi::Env env, std::string searchPath)
    {
        static constexpr auto JS_REQUIRE_FUNCTION_NAME = "require";

        if (env.Global().Get(JS_REQUIRE_FUNCTION_NAME).IsUndefined())
        {
            env.Global().Set(JS_REQUIRE_FUNCTION_NAME, Napi::Function::New(env, [searchPath = std::move(searchPath), helper = std::make_shared<RequireHelper>(env)](const Napi::CallbackInfo& info)
            {
                return helper->Execute(info.Env(), searchPath, info[0].As<Napi::String>().Utf8Value());
            }));
        }
    }
}
