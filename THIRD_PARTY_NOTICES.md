# Third-party notices

rsdownit source code is licensed under the MIT License. The programs below are separate works with their own licenses.

## yt-dlp

rsdownit can locate a system installation or download an official standalone release from <https://github.com/yt-dlp/yt-dlp/releases>.

yt-dlp source code is generally Unlicense, but official standalone executables combine dependencies and are distributed under GPL-3.0-or-later. Refer to the release and repository notices for the exact binary. rsdownit verifies the release SHA-256 but does not change its license.

## FFmpeg

On Windows, rsdownit can download the FFmpeg essentials archive published at <https://www.gyan.dev/ffmpeg/builds/>. FFmpeg and the build's enabled libraries determine whether LGPL or GPL terms apply. License and corresponding-source links are published with the build.

rsdownit does not bundle these tools in its source tree. Anyone redistributing an installer with the tools prepackaged must review and satisfy their licenses independently.
