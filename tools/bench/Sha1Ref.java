import java.security.MessageDigest;
import java.nio.file.Files;
public class Sha1Ref {
    public static void main(String[] args) throws Exception {
        byte[] data = Files.readAllBytes(java.nio.file.Path.of(args[0]));
        byte[] digest = MessageDigest.getInstance("SHA-1").digest(data);
        StringBuilder sb = new StringBuilder();
        for (byte b : digest) sb.append(String.format("%02x", b));
        System.out.println(sb);
    }
}
