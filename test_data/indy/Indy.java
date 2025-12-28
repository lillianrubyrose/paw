package indy;

import java.util.function.Consumer;

public class Indy {

    public static void main(String[] args) {
        Runnable r = () -> System.out.println("Hello, Runnable!");
        r.run();

        Consumer<String> c = s -> System.out.println(s);
        c.accept("Hello, Consumer!");
    }
}
