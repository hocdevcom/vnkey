cmake_minimum_required(VERSION 3.16)
project(viet-ime VERSION 0.1.0 LANGUAGES C CXX)
include(GNUInstallDirs)
if(NOT CMAKE_SYSTEM_NAME STREQUAL "Linux")
    message(FATAL_ERROR "Build the framework wrappers on Linux; core-rust also builds standalone on Windows")
endif()
option(VIET_BUILD_IBUS "Build IBus transport adapter" ON)
option(VIET_BUILD_FCITX5 "Build Fcitx5 transport adapter" ON)
find_program(CARGO_EXECUTABLE cargo)
if(NOT CARGO_EXECUTABLE)
    message(FATAL_ERROR "cargo is required to build viet-core")
endif()
set(VIET_CORE_LIBRARY "${PROJECT_SOURCE_DIR}/target/release/libviet_core.so")
add_custom_target(viet_core_build
    COMMAND ${CMAKE_COMMAND} -E env "CARGO_TARGET_DIR=${PROJECT_SOURCE_DIR}/target"
            ${CARGO_EXECUTABLE} build --release --locked --manifest-path "${PROJECT_SOURCE_DIR}/core-rust/Cargo.toml"
    BYPRODUCTS "${VIET_CORE_LIBRARY}"
    WORKING_DIRECTORY "${PROJECT_SOURCE_DIR}"
    VERBATIM)
add_library(VietCore SHARED IMPORTED GLOBAL)
set_target_properties(VietCore PROPERTIES IMPORTED_LOCATION "${VIET_CORE_LIBRARY}"
    INTERFACE_INCLUDE_DIRECTORIES "${PROJECT_SOURCE_DIR}/core-rust/include")
add_dependencies(VietCore viet_core_build)
install(FILES "${VIET_CORE_LIBRARY}" DESTINATION "${CMAKE_INSTALL_LIBDIR}")
install(FILES core-rust/include/viet_core.h DESTINATION "${CMAKE_INSTALL_INCLUDEDIR}")
if(VIET_BUILD_IBUS)
    add_subdirectory(wrappers/ibus-c)
endif()
if(VIET_BUILD_FCITX5)
    add_subdirectory(wrappers/fcitx5-cpp)
endif()
