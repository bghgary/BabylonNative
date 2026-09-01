#include <gtest/gtest.h>

#include <Babylon/Graphics/Device.h>
#include <Babylon/Graphics/GL/Texture.h>
#include <Babylon/Plugins/ExternalTexture.h>

#ifdef HAS_SHADER_COMPILER
#  include <Babylon/Plugins/ShaderCompiler.h>
#endif

#include "Helpers.h"

#include <GLES3/gl3.h>

#include <algorithm>
#include <string_view>
#include <vector>

extern Babylon::Graphics::Configuration g_deviceConfig;

namespace
{
#ifdef HAS_SHADER_COMPILER
    bool Contains(const std::vector<uint8_t>& bytes, std::string_view text)
    {
        return std::search(bytes.begin(), bytes.end(), text.begin(), text.end()) != bytes.end();
    }

    std::vector<uint8_t> CompileFragment(std::string_view fragmentShader)
    {
        constexpr std::string_view vertexShader{R"(
            #version 300 es
            in vec2 position;
            void main() { gl_Position = vec4(position, 0.0, 1.0); }
        )"};

        Babylon::Plugins::ShaderCompiler compiler{};
        return compiler.Compile(vertexShader, fragmentShader).FragmentBytes;
    }
#endif

    Babylon::Graphics::TextureT DescribeAsExternalOES(
        Babylon::Graphics::TextureT texture,
        uint32_t layers,
        Babylon::Graphics::GL::Texture::Usage usage)
    {
        return Babylon::Graphics::GL::Texture::Create({
            .Handle = texture->Handle(),
            .Width = texture->Width(),
            .Height = texture->Height(),
            .Layers = layers,
            .Format = GL_RGBA8,
            .Usage = usage,
            .Target = Babylon::Graphics::GL::Texture::Target::ExternalOES,
        });
    }
}

TEST(ExternalTextureOpenGL, AutomaticTargetIsDefault)
{
    Babylon::Graphics::Device device{g_deviceConfig};
    device.StartRenderingCurrentFrame();

    auto texture = Helpers::CreateTexture(device.GetPlatformInfo().Device, 4, 4);
    EXPECT_EQ(texture->TextureTarget(), Babylon::Graphics::GL::Texture::Target::Automatic);

    Helpers::DestroyTexture(texture);
    device.FinishRenderingCurrentFrame();
}

#ifdef HAS_SHADER_COMPILER
TEST(ExternalTextureOpenGL, ShaderCompilerPreservesExternalSamplerType)
{
    constexpr std::string_view fragmentShader{R"(
        #version 300 es
        #extension GL_OES_EGL_image_external_essl3 : require
        precision highp float;
        // samplerExternalOES regularTexture
        uniform samplerExternalOES externalTexture;
        uniform sampler2D regularTexture;
        out vec4 color;
        vec4 sampleExternal(samplerExternalOES source, vec2 uv)
        {
            return texture(source, uv);
        }
        void main()
        {
            color = sampleExternal(externalTexture, vec2(0.0)) + texture(regularTexture, vec2(0.0));
        }
    )"};

    const auto fragmentBytes = CompileFragment(fragmentShader);

    EXPECT_TRUE(Contains(fragmentBytes, "samplerExternalOES externalTexture"));
    EXPECT_TRUE(Contains(fragmentBytes, "samplerExternalOES source"));
    EXPECT_TRUE(Contains(fragmentBytes, "sampler2D regularTexture"));
}

TEST(ExternalTextureOpenGL, ShaderCompilerUsesActiveSamplerDeclaration)
{
    constexpr std::string_view fragmentShader{R"(
        #version 300 es
        #extension GL_OES_EGL_image_external_essl3 : require
        precision highp float;
        #ifdef USE_EXTERNAL
        uniform samplerExternalOES tex;
        #else
        uniform sampler2D tex;
        #endif
        out vec4 color;
        void main() { color = texture(tex, vec2(0.0)); }
    )"};

    const auto fragmentBytes = CompileFragment(fragmentShader);

    EXPECT_TRUE(Contains(fragmentBytes, "sampler2D tex"));
    EXPECT_FALSE(Contains(fragmentBytes, "samplerExternalOES tex"));
    EXPECT_FALSE(Contains(fragmentBytes, "GL_OES_EGL_image_external_essl3"));
}

TEST(ExternalTextureOpenGL, ShaderCompilerPreservesExternalSamplerList)
{
    constexpr std::string_view fragmentShader{R"(
        #version 300 es
        #extension GL_OES_EGL_image_external_essl3 : require
        precision highp float;
        uniform samplerExternalOES first, second;
        out vec4 color;
        void main()
        {
            color = texture(first, vec2(0.0)) + texture(second, vec2(0.0));
        }
    )"};

    const auto fragmentBytes = CompileFragment(fragmentShader);

    EXPECT_TRUE(Contains(fragmentBytes, "samplerExternalOES first"));
    EXPECT_TRUE(Contains(fragmentBytes, "samplerExternalOES second"));
}

TEST(ExternalTextureOpenGL, ShaderCompilerRejectsAmbiguousSamplerName)
{
    constexpr std::string_view fragmentShader{R"(
        #version 300 es
        #extension GL_OES_EGL_image_external_essl3 : require
        precision highp float;
        uniform samplerExternalOES externalTexture;
        uniform sampler2D regularTexture;
        out vec4 color;
        vec4 sampleExternal(samplerExternalOES source)
        {
            return texture(source, vec2(0.0));
        }
        vec4 sampleRegular(sampler2D source)
        {
            return texture(source, vec2(0.0));
        }
        void main()
        {
            color = sampleExternal(externalTexture) + sampleRegular(regularTexture);
        }
    )"};

    EXPECT_THROW(CompileFragment(fragmentShader), std::runtime_error);
}

TEST(ExternalTextureOpenGL, ShaderCompilerRejectsAmbiguousNon2DSamplerName)
{
    constexpr std::string_view fragmentShader{R"(
        #version 300 es
        #extension GL_OES_EGL_image_external_essl3 : require
        precision highp float;
        uniform samplerExternalOES tex;
        uniform samplerCube cube;
        out vec4 color;
        vec4 sampleCube(samplerCube tex)
        {
            return texture(tex, vec3(0.0));
        }
        void main()
        {
            color = texture(tex, vec2(0.0)) + sampleCube(cube);
        }
    )"};

    EXPECT_THROW(CompileFragment(fragmentShader), std::runtime_error);
}

TEST(ExternalTextureOpenGL, ShaderCompilerRejectsVertexExternalSampler)
{
    constexpr std::string_view vertexShader{R"(
        #version 300 es
        #extension GL_OES_EGL_image_external_essl3 : require
        uniform samplerExternalOES tex;
        in vec2 position;
        void main() { gl_Position = vec4(position, 0.0, 1.0); }
    )"};
    constexpr std::string_view fragmentShader{R"(
        #version 300 es
        precision highp float;
        out vec4 color;
        void main() { color = vec4(1.0); }
    )"};

    Babylon::Plugins::ShaderCompiler compiler{};
    EXPECT_THROW(compiler.Compile(vertexShader, fragmentShader), std::runtime_error);
}

TEST(ExternalTextureOpenGL, ShaderCompilerPreservesRenamedExternalSamplers)
{
    constexpr std::string_view fragmentShader{R"(
        #version 300 es
        #extension GL_OES_EGL_image_external_essl3 : require
        precision highp float;
        uniform samplerExternalOES ext__Texture;
        out vec4 color;
        vec4 shade(samplerExternalOES color, vec2 uv)
        {
            return texture(color, uv);
        }
        void main()
        {
            color = shade(ext__Texture, vec2(0.0));
        }
    )"};

    const auto fragmentBytes = CompileFragment(fragmentShader);

    EXPECT_TRUE(Contains(fragmentBytes, "samplerExternalOES ext_Texture"));
    EXPECT_TRUE(Contains(fragmentBytes, "samplerExternalOES color_1"));
}
#endif

TEST(ExternalTextureOpenGL, ExternalOESRejectsTextureArrays)
{
    Babylon::Graphics::Device device{g_deviceConfig};
    device.StartRenderingCurrentFrame();

    auto texture = Helpers::CreateTexture(device.GetPlatformInfo().Device, 4, 4);
    auto externalOES = DescribeAsExternalOES(texture, 2, Babylon::Graphics::GL::Texture::Usage::Sampled);

    try
    {
        Babylon::Plugins::ExternalTexture ignored{externalOES};
        (void)ignored;
        FAIL() << "Expected an array texture to be rejected";
    }
    catch (const std::runtime_error& error)
    {
        EXPECT_STREQ(error.what(), "ExternalTexture: GL_TEXTURE_EXTERNAL_OES requires exactly one layer");
    }

    Helpers::DestroyTexture(externalOES);
    Helpers::DestroyTexture(texture);
    device.FinishRenderingCurrentFrame();
}

TEST(ExternalTextureOpenGL, ExternalOESRejectsRenderTargets)
{
    Babylon::Graphics::Device device{g_deviceConfig};
    device.StartRenderingCurrentFrame();

    auto texture = Helpers::CreateTexture(device.GetPlatformInfo().Device, 4, 4);
    auto externalOES = DescribeAsExternalOES(texture, 1, Babylon::Graphics::GL::Texture::Usage::RenderTarget);

    try
    {
        Babylon::Plugins::ExternalTexture ignored{externalOES};
        (void)ignored;
        FAIL() << "Expected a render target to be rejected";
    }
    catch (const std::runtime_error& error)
    {
        EXPECT_STREQ(error.what(), "ExternalTexture: GL_TEXTURE_EXTERNAL_OES is sampled-only");
    }

    Helpers::DestroyTexture(externalOES);
    Helpers::DestroyTexture(texture);
    device.FinishRenderingCurrentFrame();
}
