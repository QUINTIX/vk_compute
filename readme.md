# Hello Vulkan Compute

a WIP Rust port of [my C version](https://github.com/QUINTIX/vkCompute) using 
[Vulkania](https://docs.rs/vulkanalia/latest/vulkanalia/), loosely following
<https://kylemayes.github.io/vulkanalia/>

## completed

- [x] initate/teardown instance
- [x] externalize device selection to `config.toml`
- [x] select compute capable command queue
- [x] initiate/teardown logical device
- [ ] select host visible & host coherent memory
- [ ] allocate and populate + teardown 16k floats for input and 16k floats for output
- [ ] bind/teardown input and output buffers
- [ ] compile compute shader from `cargo build`
- [ ] load computer shader module from compiled external SPV file
- [ ] create/teardown discriptor set layout & discriptor pool
- [ ] create/teardown pipeline & pipeline layout
- [ ] create/teardown command pool & command buffer
- [ ] dispatch command buffer
- [ ] submit to & wait for compute queue
- [ ] verify transformed output floats

---

This is free and unencumbered software released into the public domain.

Anyone is free to copy, modify, publish, use, compile, sell, or
distribute this software, either in source code form or as a compiled
binary, for any purpose, commercial or non-commercial, and by any
means.

In jurisdictions that recognize copyright laws, the author or authors
of this software dedicate any and all copyright interest in the
software to the public domain. We make this dedication for the benefit
of the public at large and to the detriment of our heirs and
successors. We intend this dedication to be an overt act of
relinquishment in perpetuity of all present and future rights to this
software under copyright law.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND,
EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT.
IN NO EVENT SHALL THE AUTHORS BE LIABLE FOR ANY CLAIM, DAMAGES OR
OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE,
ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR
OTHER DEALINGS IN THE SOFTWARE.

For more information, please refer to <http://unlicense.org/>