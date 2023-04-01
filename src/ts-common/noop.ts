/**
 * Function doing nothing, sometimes needed for placeholder values.
 *
 * Having a single `noop` function allows nice features such as being able to
 * compare it by reference. It is also theoretically more optimal though this
 * should probably never be a real issue at this scale.
 */
export default function noop(): void {
  /* do nothing! */
}
