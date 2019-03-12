#include "common.glsl"

out vec4 v_color;
out vec2 v_uv;
void main() {
  mat4 Projection = projection();
  mat4 Camera = camera();
  gl_Position = Projection * Camera * transpose(vec3(0, -1, 0)) *
                mat4(rotX(0.5 * PI)) * vec4(1000. * quad(vertexId), 0, 1);
  v_uv = quad_uv(vertexId);
}