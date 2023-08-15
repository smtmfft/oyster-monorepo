cmake_minimum_required(VERSION 3.13 FATAL_ERROR)
project(raw-proxy VERSION 0.0.1 LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 20)

set(CMAKE_POSITION_INDEPENDENT_CODE ON)

if("${CMAKE_BUILD_TYPE}" STREQUAL "Release")
	cmake_policy(SET CMP0069 NEW)
	set(CMAKE_POLICY_DEFAULT_CMP0069 NEW)
	set(CMAKE_INTERPROCEDURAL_OPTIMIZATION TRUE)
else()
	add_compile_options(-fsanitize=address -fsanitize=undefined)
	add_link_options(-fsanitize=address -fsanitize=undefined)
endif()

add_subdirectory(vendor EXCLUDE_FROM_ALL)

add_executable(vsock-to-ip-raw src/vsock_to_ip_raw.cpp)
target_compile_options(vsock-to-ip-raw PRIVATE -Werror -Wall -Wextra -pedantic-errors)
target_link_libraries(vsock-to-ip-raw PUBLIC absl::flat_hash_map)

if("${CMAKE_BUILD_TYPE}" STREQUAL "Release")
  target_link_options(vsock-to-ip-raw PRIVATE -static)
endif()

add_executable(ip-to-vsock-raw src/ip_to_vsock_raw.cpp)
target_compile_options(ip-to-vsock-raw PRIVATE -Werror -Wall -Wextra -pedantic-errors)

if("${CMAKE_BUILD_TYPE}" STREQUAL "Release")
  target_link_options(ip-to-vsock-raw PRIVATE -static)
endif()

