//--------/// COMPUTE ///---------//
const tau = 6.283185307179586;
const dt = 0.01;
const r = 2.0;

fn dir(a: f32) -> vec2f { return vec2f(cos(a), sin(a)); }
fn length2(v: vec2f) -> f32 { return v.x * v.x + v.y * v.y; }

struct Particle {
	u: vec2f,
	du: vec2f,
};

struct Params {
	r: f32, // radius of the magnets from centre
	d: f32,
	mu: f32, // coefficient of friction
	c: f32,
	w: u32,
	h: u32
}

@group(0) @binding(0)
var<uniform> params: Params;

@group(0) @binding(1) 
var<storage, read_write> particles: array<Particle>;

@group(0) @binding(2) 
var tex: texture_storage_2d<rgba8unorm, write>;

@compute @workgroup_size(16, 16, 1)  // PARTICLES PER GROUP: 256
fn comp_main(@builtin(global_invocation_id) global_id: vec3<u32>) {
	if (global_id.x >= params.w || global_id.y >= params.h) { return; }
	let globalidx = global_id.x + global_id.y * params.w; 

	var MAGS = array(
		r * dir(0.0),
		r * dir(tau/3),
		r * dir(2*tau/3),
	);
	var COLS: array<vec4f, 3> = array(
		vec4f(f32(0xe7)/255.0, f32(0x32)/255.0, f32(0x13)/255.0, 1.0), 
		vec4f(f32(0x9d)/255.0, f32(0xbe)/255.0, f32(0xb7)/255.0, 1.0), 
		vec4f(f32(0xef)/255.0, f32(0xe6)/255.0, f32(0xd5)/255.0, 1.0),
	);
	var p = particles[globalidx];
	var ddu = vec2f(0.0, 0.0);

	let d2 = params.d * params.d;
	for (var i=0; i<3; i++) {
		let diff = MAGS[i]-p.u;
		let sq = sqrt(length2(diff)+d2);
		ddu += diff / (sq*sq*sq);
	}
	ddu -= params.mu * p.du + params.c * p.u;

	p.du += ddu * dt;
	p.u += p.du * dt;

	particles[globalidx] = p;

	var ans = 0;
	var mind = length2(p.u-MAGS[0]);
	for (var i=1; i<3; i++) {
		let cd = length2(p.u-MAGS[i]);
		if  (cd < mind) {
			ans = i;
			mind = cd;
		}
	}

	textureStore(tex, vec2i(global_id.xy), COLS[ans]);
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

// --------/// FRAGEMENT ///---------//
@group(0) @binding(0)
var t_diffuse: texture_2d<f32>;
@group(0) @binding(1)
var s_diffuse: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
	return textureSample(t_diffuse, s_diffuse, in.tex_coords);
}