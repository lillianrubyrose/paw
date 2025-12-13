interface Meow {
    void meow();
}

public class HelloWorld implements Meow {

    public static final String HELLO_WORLD = "Hello, World!";

    public static void main(String args[]) {
        System.out.println(HELLO_WORLD);
    }

    public void meow() {}
}
