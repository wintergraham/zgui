
// Signed distance (negative inside) from a point to a box whose corner
// toward it has `radius` and `smoothing` (0 to 1): `q` is the point's offset
// from the box's centre, made positive, and `half` the box's half size.
// Smoothing 0 is the circle; above, the corner is a superellipse quadrant
// running `radius·(1+smoothing)` along each edge, its exponent keeping the
// circle's 45° point (zgui's `decoration::corner_shape`, whose
// `corner_distance` computes the same). Appended to draw.wgsl and blur.wgsl.
fn corner_distance(q:vec2<f32>, half:vec2<f32>, radius:f32, smoothing:f32)->f32 {
 let limit=max(min(half.x,half.y),0.);
 let r=clamp(radius,0.,limit);
 let extent=min(r*(1.+clamp(smoothing,0.,1.)),limit);
 let d=q-half+vec2(extent);
 if extent>r*1.0001 && d.x>0. && d.y>0. {
   // In the curve: the superellipse's norm, less its extent, over the
   // norm's gradient. With u the smaller part over the larger, the norm is
   // the larger times (1+uⁿ)^(1/n) and its gradient's length √(1+u^(2n−2))
   // over (1+uⁿ)^((n−1)/n): two powers, and the exponent only here.
   let n=-0.69314718/log(1.-0.29289322*r/extent);
   let most=max(d.x,d.y);
   let u=min(d.x,d.y)/most;
   let un1=pow(u,n-1.);
   let s=1.+un1*u;
   let a=pow(s,1./n);
   return (most*a-extent)*s/(a*sqrt(1.+un1*un1));
 }
 return length(max(d,vec2(0.)))+min(max(d.x,d.y),0.)-extent;
}
