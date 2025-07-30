<div align="center">

# GPU Magnetic Pendulum Simulation

</div>

A real-time GPU-accelerated simulation of a magnetic pendulum system that generates beautiful fractal-like patterns through chaotic dynamics.

**[Try it live in your browser (requires WebGPU)](https://rohan-t144.github.io/gpu-magnetic-pendulum/)**

## What is this?

This simulation models a magnetic pendulum suspended above multiple fixed magnets. As the pendulum swings, it experiences attractive forces from each magnet, creating complex chaotic motion. The resulting trajectories form intricate fractal patterns that reveal the underlying mathematical beauty of chaotic systems.

Each pixel in the visualization represents a particle starting from that position, with colors indicating the final position of that particle.

## Features

- **GPU-accelerated**: Utilizes compute shaders for real-time simulation of hundreds of thousands of particles
- **Interactive controls**: Adjust physics parameters and see immediate visual feedback
- **Multiple velocity patterns**: Radial, tangential, uniform, or zero initial velocities
- **Preset configurations**: Quick access to interesting parameter combinations
- **Cross-platform**: Runs natively on desktop and in web browsers via WebGPU

## Running the Simulation

Requires Rust and `cargo` to build.

### Desktop (Native)

```bash
# Clone the repository
git clone https://github.com/yourusername/gpu-magnetic-pendulum
cd gpu-magnetic-pendulum

# Run with cargo
cargo run --release
```

### Web Version

The simulation is available online at: https://rohan-t144.github.io/gpu-magnetic-pendulum/

To build for web locally:

```bash
# Install trunk if you haven't already
cargo install --locked trunk

# Build and serve
trunk serve
```

## Controls

### Simulation Parameters

- **Number of magnets** (3-10): More magnets create more complex patterns
- **Magnet radius**: Distance of magnets from the center
- **Distance parameter**: Controls singularity smoothing (affects chaos level)
- **Friction coefficient**: Higher values create smoother, less chaotic patterns
- **Spring constant**: Restoring force strength
- **Time step**: Simulation precision (smaller = more accurate)

### Initial Velocity Settings

- **Magnitude**: How fast particles start moving
- **Angle**: Rotation offset for velocity directions
- **Pattern**: How velocities are distributed:
  - **Radial**: Velocities point outward from center
  - **Tangential**: Velocities perpendicular to position (circular motion)
  - **Uniform**: All particles move in the same direction
  - **Zero**: Particles start at rest


## Math & Physics Background

The simulation implements the differential equation of motion for a damped magnetic pendulum on a GPU:
```math
\frac{d^2\mathbf{u}}{dt^2}
= \sum_{i=1}^{N}
    \frac{
        \mathbf{m}_i - \mathbf{u}
    }{
        \left( \left\| \mathbf{m}_i - \mathbf{u} \right\|^2 + d^2 \right)^{3/2}
    }
    - \mu \frac{d\mathbf{u}}{dt}
    - c\,\mathbf{u}
```

Where:
- $\mathbf{u}$ is the position vector of the particle
- $\mathbf{m}_i = r\langle \cos(2\pi i / N), \sin(2\pi i / N)\rangle$ is the position of the $i$-th magnet (with $N$ magnets arranged in a circle of radius $r$)
- $d$ is the vertical distance
- $\mu$ is the friction coefficient
- $c$ is the spring constant

The chaotic nature arises from the nonlinear interaction between multiple attractors (magnets), making the system highly sensitive to initial conditions - see chaos theory.

## Technical Implementation

- **Language**: Rust
- **Graphics**: wgpu with compute shaders
- **UI**: egui/eframe
- **Compute**: WGSL shaders for particle simulation
- **Rendering**: Real-time texture generation and display
- **Colormap**: Twilight colormap for visualization

The simulation uses a compute shader to update particle positions in parallel on the GPU, achieving real-time performance for 800×800 = 640,000 particles, and likely much higher resolutions.
