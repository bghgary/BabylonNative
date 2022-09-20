set(SOURCES 
    "Include/Babylon/Polyfills/Require.h"
    "Source/Require.cpp")

add_library(Require ${SOURCES})
warnings_as_errors(Require)

target_include_directories(Require PUBLIC "Include")

target_link_to_dependencies(Require
    PUBLIC JsRuntime
    PRIVATE arcana
    PRIVATE UrlLib)

set_property(TARGET Require PROPERTY FOLDER Polyfills)
source_group(TREE ${CMAKE_CURRENT_SOURCE_DIR} FILES ${SOURCES})
