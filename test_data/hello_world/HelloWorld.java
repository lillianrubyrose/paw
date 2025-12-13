import java.util.concurrent.ThreadLocalRandom;

interface Meow<Bark> {
    Bark meow();
}

public class HelloWorld implements Meow<Void> {

    public static final String HELLO_WORLD = "Hello, World!";

    public static void main(String args[]) {
        int abcd = 10;
        abcd += ThreadLocalRandom.current().nextInt();
        System.out.println(HELLO_WORLD);
        System.out.println(abcd);
    }

    public Void meow() {
        return null;
    }
}
