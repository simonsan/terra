#line 2

uniform vec3 cameraPosition;
uniform vec3 sunDirection;

uniform sampler2D albedo;

in vec3 fPosition;
in vec3 fColor;
in vec2 fTexcoord;

out vec4 OutColor;

void main() {
	float r = length(fPosition - cameraPosition)/2048;
	float h = random(gl_FragCoord.xy + cameraPosition.xz);

	if(smoothstep(0.8, 0.95, r) > h)
		discard;

	OutColor = texture(albedo, fTexcoord);
	if(OutColor.a < 0.5) discard;
	OutColor.rgb *= fColor / OutColor.a;
	OutColor.a = 1.0;
	
	OutColor.rgb = precomputed_aerial_perspective(OutColor.rgb, fPosition, cameraPosition, sunDirection);
}
