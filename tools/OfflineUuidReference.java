import java.nio.charset.StandardCharsets;
import java.util.UUID;

/**
 * Cross-checks the launcher's offline UUID implementation against Java's own
 * `UUID.nameUUIDFromBytes`, which is exactly what Minecraft uses in
 * `net.minecraft.util.Util#generateOfflinePlayerUuid`.
 */
public class OfflineUuidReference {

	public static void main(String[] args) {
		String[] names = args.length > 0 ? args : new String[] { "Notch", "Steve", "Player123", "aether_test" };
		for (String name : names) {
			UUID uuid = UUID.nameUUIDFromBytes(("OfflinePlayer:" + name).getBytes(StandardCharsets.UTF_8));
			System.out.println(name + "\t" + uuid);
		}
	}
}
