cmake_minimum_required(VERSION 3.14)
project(TokenStudioProofGateUiPlugin LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

if(DEFINED ENV{LOGOS_MODULE_BUILDER_ROOT})
    include($ENV{LOGOS_MODULE_BUILDER_ROOT}/cmake/LogosModule.cmake)
else()
    message(FATAL_ERROR "LogosModule.cmake not found. Set LOGOS_MODULE_BUILDER_ROOT.")
endif()

logos_module(
    NAME tokenstudio_proofgate_ui
    REP_FILE src/proofgate_ui.rep
    SOURCES
        src/proofgate_ui_backend.h
        src/proofgate_ui_backend.cpp
    INCLUDE_DIRS
        src
)
