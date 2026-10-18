//! Transcendental functions from `libm`, so that native builds and the
//! WebAssembly build compute bit-identical results: the network is chaotic,
//! and a last-digit difference grows into a different game.

pub fn exp(x: f32) -> f32 {
    libm::expf(x)
}

pub fn ln(x: f32) -> f32 {
    libm::logf(x)
}

pub fn cos(x: f32) -> f32 {
    libm::cosf(x)
}

pub fn sin(x: f32) -> f32 {
    libm::sinf(x)
}

pub fn hypot(x: f32, y: f32) -> f32 {
    libm::hypotf(x, y)
}
