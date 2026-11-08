# Concat — Performance Benchmarks & Hardware Budget Specification

> **Target Platform**: Windows 10/11 64-bit  
> **Reference Hardware**: Intel Core i3 10th Gen (2 cores / 4 threads or 4 cores / 8 threads), 8 GB RAM (4 GB soldered + 4 GB DDR4-2666), Intel UHD Graphics 620/630  
> **Memory Headroom Constraint**: Host OS + Windows Desktop Window Manager consume ~2.8–3.2 GB. Available ceiling for Concat process: **2.0–2.5 GB max**.  

---

## 1. Subsystem Memory Budgets (8 GB Low-Memory Mode)

| Subsystem | Allocation Limit | Strategy & Invariants |
|---|---:|---|
| **Host Application & Slint UI** | 350 MB | Slint Skia software / WGPU surface, unbloated component hierarchy |
| **Timeline & Project State** | 150 MB | Structural sharing snapshots (`Arc<Timeline>`), arena generational indices |
| **Video ReaderPool & Frame Cache** | 512 MB | LRU eviction, dynamic clamping via `GlobalMemoryStatusEx`, 10s idle closure |
| **Audio Buffers & Scrub PCM** | 100 MB | 48 kHz 16-bit stereo memory-mapped WAV cache; 80ms bounded scrub snippets |
| **Compositor Textures & GPU Buffers** | 300 MB | 1080p preview swapchains, 540p compute textures for scopes |
| **Export Encoder & Filtergraphs** | 400 MB | Streaming packet-by-packet pipeline, zero uncompressed video buffering |
| **Total Concat Working Set** | **~1.8 GB** | Well within the 2.5 GB safety envelope to prevent Windows page file thrashing |

---

## 2. Benchmark Profiles & Scalability Matrix

| Benchmark Scenario | Clip Count | Tracks | Playhead Scrub Latency | Target FPS | Expected Peak Memory |
|---|---:|---:|---:|---:|---:|
| **Podcast / Quick Cut** | 50 | 2V, 4A | < 16ms | 60 fps | 650 MB |
| **Short Film / Indie Edit** | 200 | 4V, 8A | < 25ms | 60 fps | 1,100 MB |
| **Complex Feature Sequence** | 500 | 8V, 16A | < 35ms | 60 fps (proxies) | 1,450 MB |
| **Stress Test Timeline** | 1,000 | 16V, 32A | < 50ms | 30–60 fps | 1,850 MB |

---

## 3. Top 5 Hardware Risks & Mitigation Engine

1. **Integrated GPU VRAM Starvation**:
   - *Risk*: Intel integrated GPUs share system RAM as dynamic video memory. Large 4K texture allocations trigger host OS out-of-memory crashes.
   - *Mitigation*: Automatic 720p proxy generation on media > 1080p; scopes run on 540p downsampled render targets.
2. **Decoder CPU Saturation**:
   - *Risk*: Decoding 4K 10-bit H.265 on an Intel i3 saturates 100% of CPU cores, dropping playback to < 15 fps.
   - *Mitigation*: Hardware decode via Intel QuickSync (D3D11VA / QSV) with automatic CPU ProRes Proxy fallback.
3. **Audio Playback Underflow During Scrub**:
   - *Risk*: Rapid mouse drag triggering thousands of seek/audio decode operations causing CPAL buffer starvation and audible buzzing/stutter.
   - *Mitigation*: Monotonic scrub generation counters dropping stale scrub events; bounded 80ms audio snippets; zero-allocation PCM cache reads.
4. **Memory Fragmentation in Long Sessions**:
   - *Risk*: Editing for 4+ hours accumulates memory fragments in the Windows native heap.
   - *Mitigation*: Periodic invocation of `HeapCompact` on Windows; closing idle media decoders after 10s of inactivity.
5. **AI Inference Memory Spikes**:
   - *Risk*: Loading Whisper and ONNX models simultaneously exceeds 8 GB RAM limit.
   - *Mitigation*: AI models are opt-in only; quantized INT8 models; automatic unload from RAM immediately following inference completion.
