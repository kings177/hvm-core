add = λa λb (+ a b)

sum = λn match n {
  0: 1
  1+: (add (sum n-1) (sum n-1))
}

main = (sum 23)
