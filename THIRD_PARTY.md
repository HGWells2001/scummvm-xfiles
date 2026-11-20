# References and attribution

The NeoAccess structure reader was adapted from the documented layout in
`port/src/hdb.rs` in [Asterber/shiloh](https://github.com/Asterber/shiloh), commit
`e3dfde0d098b25135a5243839d44842ccbc5fe91`. The bounded C++ parser adds explicit
size, pointer, tree-cycle and class-count validation. Shiloh's full runtime has
not been ported into the C++ research reader. The optional `bridge/` backend
statically links the pinned Shiloh library with its SDL frontend disabled. Its
additional required notices are in `bridge/SHILOH-NOTICES.txt` and are shipped
with runtime builds. The asset descriptor format additionally references
`port/src/media/asset.rs`; the four intro AssetRef IDs and their order reference
`port/src/front/intro.rs`, at the same pinned revision. The frontend is new C++
code using ScummVM's video/audio APIs. Shiloh's MIT notice is reproduced below.

VCAction, condition and procedure layouts additionally reference
`port/src/world/action.rs` at the same pinned revision.

The Windows GUI package also includes a minimal FFmpeg 8.0 executable for local
QDMC-to-PCM preparation, built from commit
140fd653aed8cad774f991ba083e2d01e86420c7 of https://github.com/FFmpeg/FFmpeg.
It is built without GPL/nonfree options, with the MOV demuxer, QDMC decoder,
PCM encoder and required audio conversion filters. Its LGPL 2.1 license and
source/build reference are shipped beside xfiles-audio-decoder.exe; the exact
rebuild command is tools/build_audio_decoder.sh in this repository. No game
files or prepared audio are included in that executable or distributed here.

Shiloh notice:

MIT License

Copyright (c) 2026 The Shiloh contributors

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.

