module test.module {
    requires java.base;
    requires static java.sql;
    requires transitive java.logging;
    exports test;
    opens test to java.sql;
    uses test.ModInterface;
    provides test.ModInterface with test.ModInterfaceImpl;
}
