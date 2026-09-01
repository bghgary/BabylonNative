#include <Babylon/Plugins/ShaderCompiler.h>

#include "ShaderCompilerCommon.h"
#include "ShaderCompilerTraversers.h"
#include <arcana/experimental/array.h>
#include <glslang/Public/ShaderLang.h>
#include <glslang/Public/ResourceLimits.h>
#include <SPIRV/GlslangToSpv.h>
#include <spirv_parser.hpp>
#include <spirv_glsl.hpp>

namespace
{
    void AddShader(glslang::TProgram& program, glslang::TShader& shader, std::string_view source)
    {
        const std::array<const char*, 1> sources{source.data()};
        shader.setStrings(sources.data(), gsl::narrow_cast<int>(sources.size()));

        auto defaultTBuiltInResource = GetDefaultResources();

        if (!shader.parse(defaultTBuiltInResource, 310, EProfile::EEsProfile, true, true, EShMsgDefault))
        {
            throw std::runtime_error(shader.getInfoLog());
        }

        program.addShader(&shader);
    }

    std::pair<std::unique_ptr<spirv_cross::Parser>, std::unique_ptr<spirv_cross::Compiler>> CompileShader(
        glslang::TProgram& program,
        EShLanguage stage,
        std::string& glsl,
        const std::set<std::string>& externalSamplerNames = {})
    {
        std::vector<uint32_t> spirv;
        glslang::GlslangToSpv(*program.getIntermediate(stage), spirv);

        auto parser = std::make_unique<spirv_cross::Parser>(std::move(spirv));
        parser->parse();

        const auto createCompiler = [&parser]() {
            auto compiler = std::make_unique<spirv_cross::CompilerGLSL>(parser->get_parsed_ir());
            auto options = compiler->get_common_options();
            options.version = 300;
            options.es = true;
            compiler->set_common_options(options);
            return compiler;
        };

        auto compiler = createCompiler();

        if (!externalSamplerNames.empty())
        {
            compiler->compile();

            std::set<std::string> emittedExternalSamplerNames{};
            for (const auto& metadata : parser->get_parsed_ir().meta)
            {
                const auto id = metadata.first;
                if (externalSamplerNames.contains(parser->get_parsed_ir().get_name(id)))
                {
                    emittedExternalSamplerNames.emplace(compiler->get_name(id));
                }
            }

            compiler = createCompiler();
            compiler->require_extension("GL_OES_EGL_image_external_essl3");
            compiler->set_variable_type_remap_callback(
                [emittedExternalSamplerNames = std::move(emittedExternalSamplerNames)](
                    const spirv_cross::SPIRType& type,
                    const std::string& variableName,
                    std::string& typeName) {
                    if (type.basetype == spirv_cross::SPIRType::SampledImage &&
                        emittedExternalSamplerNames.contains(variableName))
                    {
                        typeName = "samplerExternalOES";
                    }
                });
        }

        glsl = compiler->compile();

        return {std::move(parser), std::move(compiler)};
    }
}

namespace Babylon::Plugins
{
    using namespace ShaderCompilerCommon;

    ShaderCompiler::ShaderCompiler()
    {
        glslang::InitializeProcess();
    }

    ShaderCompiler::~ShaderCompiler()
    {
        glslang::FinalizeProcess();
    }

    Graphics::BgfxShaderInfo ShaderCompiler::Compile(std::string_view vertexSource, std::string_view fragmentSource, const std::map<std::string, uint32_t>& instancedAttributes)
    {
        glslang::TProgram program;

        glslang::TShader vertexShader{EShLangVertex};
        AddShader(program, vertexShader, vertexSource);

        glslang::TShader fragmentShader{EShLangFragment};
        AddShader(program, fragmentShader, fragmentSource);

        glslang::SpvVersion spv{};
        spv.spv = 0x10000;
        vertexShader.getIntermediate()->setSpv(spv);
        fragmentShader.getIntermediate()->setSpv(spv);

        if (!program.link(EShMsgDefault))
        {
            throw std::runtime_error{program.getInfoLog()};
        }

        ShaderCompilerTraversers::IdGenerator ids{};
        auto cutScope = ShaderCompilerTraversers::ChangeUniformTypes(program, ids);
        if (!ShaderCompilerTraversers::ConvertExternalSamplersTo2D(program, EShLangVertex).empty())
        {
            throw std::runtime_error{"ShaderCompiler: samplerExternalOES is only supported in fragment shaders"};
        }
        const auto externalSamplerNames = ShaderCompilerTraversers::ConvertExternalSamplersTo2D(program, EShLangFragment);
        std::map<std::string, std::string> vertexAttributeRenaming = {};
        auto builtInInstanceDataSlots = ShaderCompilerTraversers::AssignLocationsAndNamesToVertexVaryingsOpenGL(program, ids, vertexAttributeRenaming, instancedAttributes);

        std::string vertexGLSL(vertexSource.data(), vertexSource.size());
        auto [vertexParser, vertexCompiler] = CompileShader(program, EShLangVertex, vertexGLSL);

        std::string fragmentGLSL(fragmentSource.data(), fragmentSource.size());
        auto [fragmentParser, fragmentCompiler] = CompileShader(program, EShLangFragment, fragmentGLSL, externalSamplerNames);

        return CreateBgfxShader(
            {std::move(vertexParser), std::move(vertexCompiler), gsl::make_span(reinterpret_cast<uint8_t*>(vertexGLSL.data()), vertexGLSL.size()), std::move(vertexAttributeRenaming)},
            {std::move(fragmentParser), std::move(fragmentCompiler), gsl::make_span(reinterpret_cast<uint8_t*>(fragmentGLSL.data()), fragmentGLSL.size()), {}},
            std::move(builtInInstanceDataSlots));
    }
}
