cmake_minimum_required(VERSION 3.21)
project(BabylonLiteRust LANGUAGES C CXX)

# Builds LiteLayer and the runtime shader compiler for babylon-lite-sys. The
# probe executable exists only so CMake computes the complete static link line,
# which build.rs reads back through the CMake file API.

set(BABYLON_NATIVE_SOURCE_DIR "" CACHE PATH "BabylonNative repository root.")
if(NOT EXISTS "${BABYLON_NATIVE_SOURCE_DIR}/Core/LiteLayer/Include/babylon_lite.h")
    message(FATAL_ERROR "BABYLON_NATIVE_SOURCE_DIR must point at the BabylonNative repository root.")
endif()

# LiteShaderCompiler declares Dawn with FetchContent's default of recursively
# cloning every submodule (SwiftShader, ANGLE, the WebGPU CTS, ...). Declaring the
# same pin first without submodules wins, and DAWN_FETCH_DEPENDENCIES, which that
# file enables, then fetches only what Tint needs.
set(LITE_COMPILER_CMAKE "${BABYLON_NATIVE_SOURCE_DIR}/Core/LiteShaderCompiler/CMakeLists.txt")
file(READ "${LITE_COMPILER_CMAKE}" LITE_COMPILER_CMAKE_TEXT)
if(NOT LITE_COMPILER_CMAKE_TEXT MATCHES
    "FetchContent_Declare\\(LiteTint[ \t\r\n]+GIT_REPOSITORY[ \t]+([^ \t\r\n]+)[ \t\r\n]+GIT_TAG[ \t]+([0-9a-f]+)")
    message(FATAL_ERROR "Could not find the LiteTint declaration in ${LITE_COMPILER_CMAKE}.")
endif()
include(FetchContent)
FetchContent_Declare(LiteTint
    GIT_REPOSITORY "${CMAKE_MATCH_1}"
    GIT_TAG "${CMAKE_MATCH_2}"
    GIT_SUBMODULES ""
    EXCLUDE_FROM_ALL)

set(BABYLON_NATIVE_BUILD_APPS OFF CACHE BOOL "" FORCE)
set(BABYLON_NATIVE_INSTALL OFF CACHE BOOL "" FORCE)
set(BABYLON_NATIVE_BUILD_LITE_LAYER ON CACHE BOOL "" FORCE)
set(BABYLON_NATIVE_BUILD_LITE_SHADER_COMPILER ON CACHE BOOL "" FORCE)

add_subdirectory("${BABYLON_NATIVE_SOURCE_DIR}" BabylonNative EXCLUDE_FROM_ALL)

add_executable(LiteLinkProbe LinkProbe.c)
target_link_libraries(LiteLinkProbe PRIVATE LiteLayer LiteShaderCompiler)
