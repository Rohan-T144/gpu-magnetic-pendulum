//--------/// COMPUTE ///---------//
const tau = 6.283185307179586;
const pi = 3.14159265359;

fn dir(a: f32) -> vec2f { return vec2f(cos(a), sin(a)); }
fn length2(v: vec2f) -> f32 { return v.x * v.x + v.y * v.y; }

struct Particle {
	u: vec2f,
	du: vec2f,
};

struct Params {
	n: u32,
	r: f32, // radius of the magnets from centre
	d: f32,
	mu: f32, // coefficient of friction
	c: f32,
	dt: f32,
	w: u32,
	h: u32,
}

@group(0) @binding(0)
var<uniform> params: Params;

@group(0) @binding(1) 
var<storage, read_write> particles: array<Particle>;

@group(0) @binding(2) 
var tex: texture_storage_2d<rgba8unorm, write>;

@group(0) @binding(3)
var<storage, read> colormap: array<vec4<f32>>;

// we need a cyclic colormap (check out a way to use the twilight one).
// fn color(l: f32) -> vec4f {
	// return vec4f(colormap[50], 1.0);
	// let s = clamp(atan(l) / (tau/4), 0.0, 1.0); // 0..pi/2 -> 0..1
	// let n = 500;
	// let if32 = s * f32(n);
	// var f = saturate(atan(l) / (tau/4));
	// f = modf(f+0.5).fract;
	// let i = u32(floor(f * 511));
	// return vec4f(colormap[i], 1.0);
	// return vec4f(atan(l) / (tau/4), atan(l) / (tau/4), atan(l) / (tau/4), 1.0);
	// let a = atan(l) * 2; // 0..pi/2 -> 0..pi
	// let s = cos(a) * cos(a);

	// return vec4f(s, s, s, 1.0);
// }

@compute @workgroup_size(16, 16, 1)  // PARTICLES PER GROUP: 256
fn comp_main(@builtin(global_invocation_id) global_id: vec3<u32>) {
	if (global_id.x >= params.w || global_id.y >= params.h) { return; }
	let globalidx = global_id.x + global_id.y * params.w; 

	var p = particles[globalidx];
	var ddu = vec2f(0.0, 0.0);

	let d2 = params.d * params.d;
	for (var i: u32 = 0; i < params.n; i++) {
		let mag = params.r * dir(f32(i)*tau/f32(params.n));
		let diff = mag-p.u;
		let sq = sqrt(length2(diff)+d2);
		ddu += diff / (sq*sq*sq);
	}
	ddu -= params.mu * p.du + params.c * p.u;

	p.du += ddu * params.dt;
	p.u += p.du * params.dt;

	particles[globalidx] = p;

	let a = atan2(p.u.y, p.u.x);
	let frac = saturate((a+pi) / tau);
	// let frac = f32(global_id.x) / 800.0;
	//let col = vec4f(frac, frac, 0.0, 1.0);
	// let col = vec4f(colormap[255], 1.0);
	let col = colormap[u32(floor(frac * 510.0))];

	textureStore(tex, vec2i(global_id.xy), col);
}

//--------/// VERTEX ///---------//
struct VertexOutput {
	@builtin(position) clip_position: vec4<f32>,
	@location(0) tex_coords: vec2<f32>,
}

@vertex
fn vs_main(
	@builtin(vertex_index) vid: u32,
	@location(0) clip_pos: vec2f,
	@location(1) tex_pos: vec2f,
) -> VertexOutput {
	var out: VertexOutput;
	out.tex_coords = tex_pos;
	out.clip_position = vec4f(clip_pos, 0.0, 1.0);
	return out;
}

// --------/// FRAGMENT ///---------//
@group(0) @binding(0)
var t_diffuse: texture_2d<f32>;
@group(0) @binding(1)
var s_diffuse: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
	return textureSample(t_diffuse, s_diffuse, in.tex_coords);
}
