cmake_minimum_required(VERSION 3.13 FATAL_ERROR)
project(raw-proxy VERSION 0.0.1 LANGUAGES CXX)

set(CMAKE_POSITION_INDEPENDENT_CODE ON)

if("${CMAKE_BUILD_TYPE}" STREQUAL "Release")
	cmake_policy(SET CMP0069 NEW)
	set(CMAKE_POLICY_DEFAULT_CMP0069 NEW)
	set(CMAKE_INTERPROCEDURAL_OPTIMIZATION TRUE)
else()
	add_compile_options(-fsanitize=address -fsanitize=undefined)
	add_link_options(-fsanitize=address -fsanitize=undefined)
endif()

