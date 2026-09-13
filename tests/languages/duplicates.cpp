namespace math {
class Calculator {
public:
    int first(int value) { int result = value * 2; if (result > 0) return result; return 0; }
    int second(int input) { int output = input * 2; if (output > 0) return output; return 0; }
};
template <typename T> T twice(T value) { return value + value; }
auto increment = [](int value) { return value + 1; };
}
