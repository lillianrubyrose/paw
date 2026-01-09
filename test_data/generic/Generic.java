import java.io.Serializable;
import java.util.Map;
import java.util.function.Consumer;

public class Generic<T, E extends Number> implements Consumer<E> {

    private T field;
    private Map<? super Consumer<?>, T> map;

    public static void main(String[] args) {}

    @Override
    public void accept(E arg0) {
        throw new UnsupportedOperationException(
            "Unimplemented method 'accept'"
        );
    }

    public class Inner<F> implements Serializable {

        private E innerField;
    }
}
