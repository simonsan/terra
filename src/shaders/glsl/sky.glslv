out vec2 position;

void main() {
	if(gl_VertexID == 0) gl_Position = vec4(-1, -1, 0, 1);
	if(gl_VertexID == 1) gl_Position = vec4(-1,  3, 0, 1);
	if(gl_VertexID == 2) gl_Position = vec4( 3, -1, 0, 1);
	position = gl_Position.xy;
}
