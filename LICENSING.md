# Licensing

ClipCat is an independent desktop video editing application and derivative work based on Concat, maintained by lyric9901 and ClipCat contributors.

## 1. AGPL-3.0-or-later

The source code in this repository, and all official binary distributions built from it, is licensed under the **GNU Affero General Public License, version 3 or later (AGPL-3.0-or-later)**.

Under the terms of AGPL-3.0-or-later:
- You may freely run, inspect, modify, and redistribute ClipCat.
- If you distribute modified versions or run them over a network providing services to users, you must make the complete corresponding source code available under the AGPL-3.0-or-later.
- Existing copyright notices and licensing headers are preserved in compliance with Section 4 and Section 5 of the AGPL-3.0.

## 2. Code Classification & Provenance

In accordance with open-source licensing standards, the ClipCat codebase is classified as follows:

| Classification | Components | Licensing & Ownership |
|---|---|---|
| **ClipCat Extensions & Modifications** | Professional branding suite, 6th workspace preset, video scopes integration, Fairlight-style audio mixer, silence ripple cut, asset database & pack installer, CI/release workflows | Copyright 2026 lyric9901 and ClipCat contributors; licensed under AGPL-3.0-or-later. |
| **Inherited Core Code** | Core editing engine, timeline document model, GPU compositing pipelines, host layer | Derived from Concat; Copyright 2026 Jareer Abdullah and Concat contributors; licensed under AGPL-3.0-or-later. |
| **Application Branding** | Blue Cat Clapperboard logo and icon suite | Copyright 2026 lyric9901. All rights reserved. |
| **Bundled Typography** | Hanken Grotesk static font instances (`src/crates/concat-text/fonts/`) | SIL Open Font License (OFL-1.1). |
| **Third-Party Frameworks** | Slint UI, FFmpeg runtime libraries, ONNX Runtime, Whisper.cpp | See [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md) for individual component licenses. |

## 3. Codec & Patent Disclaimers

ClipCat does not grant rights under third-party patents covering proprietary media formats (such as H.264, HEVC, or AAC). Hardware and platform encoders/decoders provided by your operating system or graphics driver are governed by their respective vendor licenses.
