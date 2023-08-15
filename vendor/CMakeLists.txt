#-------- abseil --------#

set(BUILD_SHARED_LIBS OFF CACHE BOOL "absl shared libs")
set(ABSL_PROPAGATE_CXX_STD ON CACHE BOOL "absl prop")
add_subdirectory(abseil-cpp)

