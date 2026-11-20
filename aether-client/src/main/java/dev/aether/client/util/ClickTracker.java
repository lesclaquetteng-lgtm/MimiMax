package dev.aether.client.util;

import java.util.ArrayDeque;
import java.util.Deque;

/**
 * Counts the mouse clicks of the player.
 *
 * The click tracker is fed by a mixin on the mouse handler, which makes it independent of the
 * frame rate: every single click is recorded even when several happen inside one frame.
 */
public final class ClickTracker {
	private static final int WINDOW_MILLIS = 1000;

	private final Deque<Long> leftClicks = new ArrayDeque<>();
	private final Deque<Long> rightClicks = new ArrayDeque<>();

	/**
	 * Records a click.
	 * @param button the mouse button, 0 is the left one
	 */
	public void record(int button) {
		long now = System.currentTimeMillis();
		if (button == 0) {
			leftClicks.addLast(now);
		} else if (button == 1) {
			rightClicks.addLast(now);
		}
		prune(leftClicks, now);
		prune(rightClicks, now);
	}

	private static void prune(Deque<Long> clicks, long now) {
		while (!clicks.isEmpty() && now - clicks.peekFirst() > WINDOW_MILLIS) {
			clicks.removeFirst();
		}
	}

	/**
	 * @return the clicks per second of the left mouse button.
	 */
	public int getLeftClicksPerSecond() {
		prune(leftClicks, System.currentTimeMillis());
		return leftClicks.size();
	}

	/**
	 * @return the clicks per second of the right mouse button.
	 */
	public int getRightClicksPerSecond() {
		prune(rightClicks, System.currentTimeMillis());
		return rightClicks.size();
	}

	/**
	 * Removes every recorded click.
	 */
	public void clear() {
		leftClicks.clear();
		rightClicks.clear();
	}
}
