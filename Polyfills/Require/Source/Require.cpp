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

    Napi::Value Require(Napi::Env env, std::string searchPath, std::filesystem::path path)
    {
        static std::filesystem::path currentScriptPath;

        std::filesystem::path resolvedPath;

        if (path.native()[0] == L'.') {
            if (currentScriptPath.empty()) {
                throw Napi::Error::New(env, "Cannot load from a relative path from the root");
            }

            resolvedPath = (currentScriptPath / path).lexically_normal();
        }
        else if (path.native()[0] == L'/') {
            resolvedPath = searchPath / std::filesystem::path{path.native().substr(1)};
        }
        else
        {
            resolvedPath = (searchPath / path).lexically_normal();
        }

        if (std::filesystem::is_directory(resolvedPath))
        {
            std::filesystem::path packagePath{std::filesystem::path{searchPath} / path};
            std::filesystem::path packageJsonPath{packagePath / "package.json"};
            if (std::filesystem::exists(packageJsonPath))
            {
                Napi::Function jsonParse = env.Global().Get("JSON").As<Napi::Object>().Get("parse").As<Napi::Function>();
                Napi::Value main = jsonParse.Call({Napi::String::From(env, readFile(packageJsonPath))}).As<Napi::Object>().Get("main");
                resolvedPath = (packagePath / (main.IsUndefined() ? "index.js" : main.As<Napi::String>().Utf8Value())).lexically_normal();
            }
        }
        else if (!std::filesystem::exists(resolvedPath))
        {
            if (resolvedPath.extension() != ".js")
            {
                resolvedPath += ".js";
            }
        }

        if (resolvedPath.extension() == ".json")
        {
            Napi::Function jsonParse = env.Global().Get("JSON").As<Napi::Object>().Get("parse").As<Napi::Function>();
            return jsonParse.Call({Napi::String::From(env, readFile(resolvedPath))});
        }

        Napi::Object exports{Napi::Object::New(env)};
        env.Global().Set("exports", exports);
        auto oldScriptPath = currentScriptPath;
        currentScriptPath = resolvedPath.parent_path();
        {
            auto scope = gsl::finally([oldScriptPath = std::move(oldScriptPath), env]() {
                currentScriptPath = oldScriptPath;
                env.Global().Delete("exports");
            });

            Napi::Function::New(env, [resolvedPath = std::move(resolvedPath)](const Napi::CallbackInfo& info)
            {
                Napi::Eval(info.Env(), readFile(resolvedPath).data(), resolvedPath.string().data());
            }).Call({});
        }

        return exports;
    }
}

namespace Babylon::Polyfills::Require
{
    void Initialize(Napi::Env env, std::string searchPath)
    {
        static constexpr auto JS_REQUIRE_FUNCTION_NAME = "require";

        if (env.Global().Get(JS_REQUIRE_FUNCTION_NAME).IsUndefined())
        {
            env.Global().Set(JS_REQUIRE_FUNCTION_NAME, Napi::Function::New(env, [searchPath = std::move(searchPath)](const Napi::CallbackInfo& info) mutable
            {
                return ::Require(info.Env(), searchPath, info[0].As<Napi::String>().Utf8Value());
            }));
        }
    }
}
