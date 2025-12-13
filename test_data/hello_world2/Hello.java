import java.lang.Deprecated;
import java.lang.RuntimeException;
import java.lang.System;
import java.lang.annotation.ElementType;
import java.lang.annotation.Retention;
import java.lang.annotation.RetentionPolicy;
import java.lang.annotation.Target;
import java.util.HashMap;
import java.util.Map;
import java.util.function.Supplier;

public final class Hello {

    @Deprecated
    public static final String MESSAGE = "Hello World!";

    public static void main(String[] args) {
        System.out.println(MESSAGE);
    }

    @HelloAnnoRec(value = @HelloAnno(value = "Hi"))
    public static <T> void stackmapper(int value, T ty) {
        int i = 0;
        int j = 0;
        T ty2 = ty;
        if (i > 0) {
            long k = 0;
            if (j == 0) {
                k++;
                int s = 1111;
            }
            String t = "fucker";
        }
        Supplier<String> sup = () -> {
            return "fuck";
        };
        HelloInterface h = new HelloInterface() {
            @Override
            public void print(String value) {}
        };
    }

    @HelloAnno(value = "Meow")
    public void thrower() throws RuntimeException {}

    public interface HelloInterface {
        void print(@HelloAnno(value = "Meow") String value);
    }

    @Target(
        {
            ElementType.PARAMETER,
            ElementType.FIELD,
            ElementType.LOCAL_VARIABLE,
            ElementType.PACKAGE,
            ElementType.METHOD,
            ElementType.TYPE_PARAMETER,
            ElementType.TYPE,
            ElementType.TYPE_USE,
            ElementType.ANNOTATION_TYPE,
        }
    )
    public @interface HelloAnno {
        String value() default "WAWAWAW";
    }

    @Retention(value = RetentionPolicy.RUNTIME)
    @Target(
        {
            ElementType.PARAMETER,
            ElementType.FIELD,
            ElementType.LOCAL_VARIABLE,
            ElementType.PACKAGE,
            ElementType.METHOD,
            ElementType.TYPE_PARAMETER,
            ElementType.TYPE,
            ElementType.TYPE_USE,
            ElementType.ANNOTATION_TYPE,
        }
    )
    public @interface HelloAnnoRec {
        HelloAnno value();
    }

    @Target(
        {
            ElementType.PARAMETER,
            ElementType.FIELD,
            ElementType.LOCAL_VARIABLE,
            ElementType.PACKAGE,
            ElementType.METHOD,
            ElementType.TYPE_PARAMETER,
            ElementType.TYPE,
            ElementType.TYPE_USE,
            ElementType.ANNOTATION_TYPE,
        }
    )
    public @interface Meow {}

    public interface GenericAnnot<@Meow T> {}

    @Deprecated
    public class InnerHello<T> implements HelloInterface {

        public final Map<
            @HelloAnnoRec(value = @HelloAnno(value = "mew")) String,
            Map<Integer, @Meow T>
        > map = new HashMap<>();

        public int intMethod() {
            return -1;
        }

        public void intMethod(int value) {}

        public String stringMethod() {
            return "meow";
        }

        public void stringMethod(String value) {}

        @Override
        public void print(String value) {
            System.out.println(value);
        }
    }

    public sealed class Shape permits Circle {}

    public final class Circle extends Shape {}

    public record Cat(String name, int age) {}
}
