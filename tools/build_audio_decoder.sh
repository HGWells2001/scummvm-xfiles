#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Minimal, offline runtime helper. All game decoding remains local.
set -euo pipefail
git init ffmpeg-source
git -C ffmpeg-source remote add origin https://github.com/FFmpeg/FFmpeg.git
git -C ffmpeg-source fetch --depth=1 origin 140fd653aed8cad774f991ba083e2d01e86420c7
git -C ffmpeg-source checkout FETCH_HEAD
cd ffmpeg-source
./configure --disable-everything --disable-autodetect --disable-network \
  --disable-doc --disable-debug --disable-ffplay --disable-ffprobe \
  --disable-avdevice --disable-swscale --disable-x86asm \
  --enable-small --enable-static --disable-shared --enable-ffmpeg \
  --enable-demuxer=mov --enable-decoder=qdmc --enable-encoder=pcm_s16le \
  --enable-filter=aresample --enable-filter=aformat --enable-filter=anull \
  --enable-filter=abuffer --enable-filter=abuffersink \
  --enable-muxer=pcm_s16le --enable-protocol=file --enable-protocol=pipe \
  --extra-ldflags=-static
make -j4
mkdir -p ../decoder-package
cp ffmpeg.exe ../decoder-package/xfiles-audio-decoder.exe
cp COPYING.LGPLv2.1 LICENSE.md ../decoder-package/
printf 'FFmpeg 8.0, pinned commit 140fd653aed8cad774f991ba083e2d01e86420c7. Minimal QDMC/MOV to PCM decoder. Source: https://github.com/FFmpeg/FFmpeg/tree/140fd653aed8cad774f991ba083e2d01e86420c7 . Rebuild: tools/build_audio_decoder.sh in the ScummVM-XFiles source repository.\n' > ../decoder-package/AUDIO-DECODER-SOURCE.txt
