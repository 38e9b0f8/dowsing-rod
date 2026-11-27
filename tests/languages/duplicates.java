package math;

import java.util.List;
import java.util.function.IntUnaryOperator;

public class Calculator {
    private int base;

    public int sumPositive(List<Integer> values) {
        int total = 0;
        for (int value : values) { if (value > 0) { total += value; } }
        return total;
    }

    public int addPositive(List<Integer> items) {
        int result = 0;
        for (int item : items) { if (item > 0) { result += item; } }
        return result;
    }

    public int getBase() { return base; }

    public void setBase(int base) { this.base = base; }

    public IntUnaryOperator doubler() {
        IntUnaryOperator twice = n -> n * 2;
        return twice;
    }
}

interface Shape {
    double area();

    default String describe() { return "shape " + area(); }
}

record Point(int x, int y) {
    Point {
        if (x < 0) { throw new IllegalArgumentException("x"); }
    }
}
