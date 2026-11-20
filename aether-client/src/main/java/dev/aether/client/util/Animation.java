package dev.aether.client.util;

/**
 * A smooth, frame rate independent animation.
 *
 * The value moves towards a target with an exponential decay, which keeps the motion identical on
 * every frame rate.
 */
public class Animation {
	/** The speed the animations use when the caller does not give one: a value crosses most of its
	 * distance in about a fifth of a second, which reads as smooth without feeling slow. */
	public static final float DEFAULT_SPEED = 12.0f;

	private float value;
	private float target;
	private float speed = DEFAULT_SPEED;

	/**
	 * Creates an animation that starts where it is asked to.
	 *
	 * <p>The single argument form used to leave the speed at zero, which froze the animation: it
	 * looked alive and never moved. It now takes {@link #DEFAULT_SPEED}.</p>
	 */
	public Animation(float initial) {
		this.value = initial;
		this.target = initial;
	}

	public Animation(float initial, float speed) {
		this.value = initial;
		this.target = initial;
		this.speed = speed > 0.0f ? speed : DEFAULT_SPEED;
	}

	/**
	 * Advances the animation by the given number of milliseconds.
	 *
	 * <p>A negative or zero delta is ignored: a clock that steps backwards (a paused game, a
	 * resized window) must not throw the value the other way. The motion is exponential, so
	 * two steps of half the time land where one step of the whole time does, whatever the frame
	 * rate.</p>
	 */
	public void update(long deltaMillis) {
		if (deltaMillis <= 0L) {
			return;
		}
		float factor = 1.0f - (float) Math.pow(0.001, (deltaMillis / 1000.0) * speed);
		value = value + (target - value) * factor;
	}

	public float getSpeed() {
		return speed;
	}

	public float getValue() {
		return value;
	}

	public float getTarget() {
		return target;
	}

	public void setTarget(float target) {
		this.target = target;
	}

	/**
	 * Sets the value and the target at the same time, which stops the animation.
	 */
	public void set(float value) {
		this.value = value;
		this.target = value;
	}

	public boolean isFinished() {
		return Math.abs(value - target) < 0.001f;
	}
}
