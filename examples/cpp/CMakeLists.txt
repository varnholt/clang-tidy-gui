cmake_minimum_required(VERSION 3.16)
project(tidy_playground LANGUAGES CXX)
add_executable(tidy_playground main.cpp)
target_compile_features(tidy_playground PRIVATE cxx_std_11)
