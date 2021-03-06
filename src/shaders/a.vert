#line 2

layout(location = 0, component=0) in vec3 heights_origin;
layout(location = 0, component=3) in float heights_step;
layout(location = 1, component=0) in vec3 pheights_origin;
layout(location = 1, component=3) in float pheights_step;
layout(location = 2, component=0) in vec3 albedo_origin;
layout(location = 2, component=3) in float albedo_step;
layout(location = 3, component=0) in vec3 palbedo_origin;
layout(location = 3, component=3) in float palbedo_step;
layout(location = 4, component=0) in vec3 roughness_origin;
layout(location = 4, component=3) in float roughness_step;
layout(location = 5, component=0) in vec3 proughness_origin;
layout(location = 5, component=3) in float proughness_step;
layout(location = 6, component=0) in vec3 normals_origin;
layout(location = 6, component=3) in float normals_step;
layout(location = 7, component=0) in vec3 pnormals_origin;
layout(location = 7, component=3) in float pnormals_step;
layout(location = 8, component=0) in uint resolution;
layout(location = 8, component=1) in uint level_resolution;
layout(location = 8, component=2) in ivec2 in_position;
layout(location = 9, component=0) in uint face;
layout(location = 10, component=0) in float min_distance;

layout(set = 0, binding = 0) uniform UniformBlock {
    mat4 view_proj;
	dvec3 camera;
	double padding;
} ubo;
layout(set = 0, binding = 1) uniform sampler linear;
layout(set = 0, binding = 8) uniform texture2DArray displacements;

layout(location = 0) out vec3 out_position;
layout(location = 1) out vec3 out_albedo_texcoord;
layout(location = 2) out vec3 out_palbedo_texcoord;
layout(location = 3) out vec3 out_roughness_texcoord;
layout(location = 4) out vec3 out_proughness_texcoord;
layout(location = 5) out vec3 out_normals_texcoord;
layout(location = 6) out vec3 out_pnormals_texcoord;
layout(location = 7) out float out_morph;
layout(location = 8) out vec3 out_normal;
layout(location = 9) out vec3 out_tangent;
layout(location = 10) out vec3 out_bitangent;

layout(location = 11) out vec2 out_i_position;
// layout(location = 9) out float out_resolution;
// layout(location = 10) out float out_min_distance;
// layout(location = 11) out float out_elevation;
layout(location = 12) out float out_face;
layout(location = 13) out float out_level_resolution;


const double planetRadius = 6371000.0;

struct Positions {
	dvec2 face; // Range of [-1, 1] along a cube face
	dvec3 cube; // Ranges between [-1, 1] for all 3 axis's
	dvec3 sphere; // Position on a unit sphere
	vec3 world; // In world space
};

dvec3 cube_position(vec2 iPosition) {
	dvec2 facePosition = 2.0 * (dvec2(iPosition) + dvec2(in_position)) / double(level_resolution);

	// See "Cube-to-sphere Projections for ProceduralTexturing and Beyond"
	// http://jcgt.org/published/0007/02/01/paper.pdf
	facePosition = sign(facePosition) * (1.4511 - sqrt(1.4511 * 1.4511 - 1.8044 * abs(facePosition))) / 0.9022;

	dvec3 cubePosition = dvec3(0);
	if(face == 0) cubePosition = dvec3(1.0, facePosition.x, -facePosition.y);
	else if(face == 1) cubePosition = dvec3(-1.0, -facePosition.x, -facePosition.y);
	else if(face == 2) cubePosition = dvec3(facePosition.x, 1.0, facePosition.y);
	else if(face == 3) cubePosition = dvec3(-facePosition.x, -1.0, facePosition.y);
	else if(face == 4) cubePosition = dvec3(facePosition.x, -facePosition.y, 1.0);
    else if(face == 5) cubePosition = dvec3(-facePosition.x, -facePosition.y, -1.0);
	return cubePosition;
}

vec3 compute_local_position(vec2 iPosition, out vec3 tangent, out vec3 normal, out vec3 bitangent) {
	dvec3 spherePosition = normalize(cube_position(iPosition));
	dvec3 spherePositionDx = normalize(cube_position(iPosition+vec2(0.01)));

	normal = vec3(spherePosition);
	tangent = normalize(vec3(spherePositionDx - spherePosition));
	bitangent = normalize(cross(tangent, normal));
	tangent = normalize(cross(normal, bitangent));

	return vec3(spherePosition * planetRadius - ubo.camera.xyz);
}

float compute_morph(vec2 iPosition) {
	vec3 cubePosition = vec3(cube_position(iPosition));

	vec3 camera = vec3(ubo.camera.x, ubo.camera.y, ubo.camera.z);
	float r = max(max(abs(camera.x), abs(camera.y)), abs(camera.z));
	camera = camera / r;

	camera = camera * (1.4511 + (1 - 1.4511)*abs(camera));
	cubePosition = cubePosition * (1.4511 + (1 - 1.4511)*abs(cubePosition));

	if (min_distance == 0)
		return 0;

	return 1 - smoothstep(0.9, 1, float(distance(cubePosition, camera)) / min_distance);
}

void main() {
	ivec2 iPosition = ivec2((gl_VertexIndex) % (resolution+1),
							(gl_VertexIndex) / (resolution+1));

	vec3 tangent, normal, bitangent;
	// vec3 gridPosition = compute_local_position(iPosition, tangent, normal, bitangent);


	// float morph = 1 - smoothstep(0.7, 0.95, distance(gridPosition.xz, ubo.camera.xz) / min_distance);
	// morph = min(morph * 2, 1) * 0;
	// if(is_top_level)
	//	morph = 1;
	float morph = compute_morph(iPosition);
	vec2 nPosition = mix(vec2((iPosition / 2) * 2), vec2(iPosition), morph);

	vec3 offset = texture(sampler2DArray(displacements, linear),
						  heights_origin + vec3(vec2(nPosition) * heights_step, 0)).xyz;
	if (pheights_origin.z >= 0) {
		offset = mix(texture(sampler2DArray(displacements, linear),
							 pheights_origin + vec3(vec2(nPosition) * pheights_step, 0)).xyz,
					 offset,
					 morph);
	}

	vec3 position = compute_local_position(nPosition, tangent, normal, bitangent);
	position += mat3(tangent, normal, bitangent) * offset;

	out_position = position;
	out_albedo_texcoord = albedo_origin + vec3(nPosition * albedo_step, 0);
	out_palbedo_texcoord = palbedo_origin + vec3(nPosition * palbedo_step, 0);
	out_roughness_texcoord = roughness_origin + vec3(nPosition * roughness_step, 0);
	out_proughness_texcoord = proughness_origin + vec3(nPosition * proughness_step, 0);
	out_normals_texcoord = normals_origin + vec3(nPosition * normals_step, 0);
	out_pnormals_texcoord = pnormals_origin + vec3(nPosition * pnormals_step, 0);
	out_morph = morph;
	out_normal = normal;
	out_tangent = tangent;
	out_bitangent = bitangent;

	out_i_position = vec2(in_position + iPosition);
	// out_resolution = resolution;
	// out_min_distance = min_distance;
	// out_elevation = texture(sampler2DArray(displacements, linear),
	// 						heights_origin + vec3(nPosition * heights_step, 0)).g;
	out_face = face;
	out_level_resolution = level_resolution;
	gl_Position = ubo.view_proj * vec4(position, 1.0);
}
