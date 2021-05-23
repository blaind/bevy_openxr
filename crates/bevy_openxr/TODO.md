
# What to be implemented

* Graceful shutdown, make sure that `XRDevice` is dropped before `WGPURenderer` in bevy_wgpu crate
  * Make gpu-rs api safe?
  * Arc between two?
  * Make a test that catches println! / trace! statements from Drop impls in both?


