import java.util.*;

public class InsnTest {

    private static final double PI = 3.14159;
    private int instanceCount = 0;

    public static void main(String[] args) {
        InsnTest t = new InsnTest();
        t.executeAll();
        System.out.println("done");
    }

    public void executeAll() {
        int a = 10;
        int b = 20;
        int sum = a + b;
        double area = PI * (sum * sum);

        int bitwise = (a << 2) | 1;
        instanceCount++;

        int[] intArray = new int[5];
        intArray[0] = sum;

        String[] stringArray = new String[2];
        stringArray[0] = "Java 25";

        int[][][] threeD = new int[2][2][2];

        controlFlow(a);

        Object obj = "Hello";
        if (obj instanceof String s) {
            long length = (long) s.length();
        }

        try {
            thrower(bitwise);
        } catch (RuntimeException e) {}

        synchronized (this) {
            instanceCount--;
        }
    }

    private void controlFlow(int value) {
        switch (value) {
            case 1 -> {}
            case 2 -> {}
            case 3 -> {}
            default -> {}
        }

        switch (value) {
            case 10 -> {}
            case 1000 -> {}
            default -> {}
        }
    }

    private void thrower(int val) {
        if (val < 0) {
            throw new RuntimeException("Negative value");
        }
    }

    public List<String> collectionExample() {
        List<String> list = new ArrayList<>();
        list.add("Item");
        return list;
    }
}
