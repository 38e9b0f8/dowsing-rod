int sum_positive(int *values, int count) {
    int total = 0;
    for (int i = 0; i < count; ++i) { if (values[i] > 0) { total += values[i]; } }
    return total;
}
int add_positive(int *items, int size) {
    int result = 0;
    for (int j = 0; j < size; ++j) { if (items[j] > 0) { result += items[j]; } }
    return result;
}
